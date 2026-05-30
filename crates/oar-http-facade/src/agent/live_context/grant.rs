use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use oar_core::action::audit_event::{AuditActor, AuditActorKind};
use oar_core::action::token_refresh_audit::TokenRefreshAuditContext;
use oar_core::domain::identity::{
    ActorKind, ScopeBoundary, TenantId, TokenGrantId, TokenGrantState,
};
use oar_core::domain::token_refresh::types::{
    TokenRefreshCommandKind, TokenRefreshGrantSnapshot, TokenRefreshReportStatus,
    TokenRefreshServiceReport,
};
use oar_core::storage::postgres::{
    EncryptedTokenGrantRecord, PostgresTokenGrantRepository, PostgresTokenRefreshOrchestrator,
};
use oar_lark_adapter::build_postgres_async_feishu_auth_refresh_adapter;
use sha2::{Digest, Sha256};

use crate::{feishu_auth, AuthenticatedContext};

mod lookup;

pub(super) use lookup::{resolve_grant_id_for_user, resolve_lark_open_id_for_grant};

pub(super) const TOKEN_REFRESH_SKEW_MS: u64 = 5 * 60 * 1000;

pub(super) async fn refresh_grant_before_live_read(
    pool: sqlx::PgPool,
    login: &feishu_auth::FeishuLoginRuntime,
    persistence: &feishu_auth::FeishuGrantPersistenceRuntime,
    auth_context: &AuthenticatedContext,
    token_grant: &EncryptedTokenGrantRecord,
    now: SystemTime,
    now_ms: u64,
) -> Result<EncryptedTokenGrantRecord, LiveGrantRefreshError> {
    let adapter = build_postgres_async_feishu_auth_refresh_adapter(
        pool.clone(),
        login.open_api_config(),
        login.client_id().to_string(),
        login.client_secret(),
        persistence.grant_key_id().to_string(),
        persistence.grant_key_material(),
    )
    .map_err(|_| LiveGrantRefreshError::RefreshUnavailable)?;

    let snapshot = token_refresh_snapshot_for_live_read(token_grant);
    let audit_context = live_read_refresh_audit_context(auth_context, &token_grant.id, now_ms);
    let mut orchestrator = PostgresTokenRefreshOrchestrator::new(pool.clone(), adapter);
    let report = orchestrator
        .refresh_grant_with_audit(snapshot, now, audit_context)
        .await
        .map_err(|_| LiveGrantRefreshError::RefreshUnavailable)?;
    ensure_refresh_report_allows_read(&report.service_report)?;

    let refreshed = PostgresTokenGrantRepository::new(pool)
        .get_by_id(&auth_context.tenant_id, &token_grant.id)
        .await
        .map_err(|_| LiveGrantRefreshError::ReloadFailed)?
        .ok_or(LiveGrantRefreshError::GrantMissing)?;
    if grant_requires_refresh_before_read(&refreshed, now_ms) {
        return Err(LiveGrantRefreshError::StillStale);
    }
    Ok(refreshed)
}

pub(super) fn grant_requires_refresh_before_read(
    token_grant: &EncryptedTokenGrantRecord,
    now_ms: u64,
) -> bool {
    matches!(
        token_grant.state,
        TokenGrantState::NeedsRefresh | TokenGrantState::Expired
    ) || token_grant
        .expires_at_ms
        .map(|expires_at_ms| expires_at_ms <= now_ms.saturating_add(TOKEN_REFRESH_SKEW_MS))
        .unwrap_or(false)
}

pub(super) fn live_read_grant_denial_reason(
    token_grant: &EncryptedTokenGrantRecord,
) -> Option<&'static str> {
    if matches!(
        token_grant.state,
        TokenGrantState::Revoked | TokenGrantState::ReauthRequired
    ) || token_grant.reauth_required_at_ms.is_some()
        || token_grant.revoked_at_ms.is_some()
    {
        return Some("授权已失效，需要重新登录");
    }
    if token_grant.actor_kind != ActorKind::User
        || token_grant.scope_boundary != ScopeBoundary::User
    {
        return Some("授权主体不是当前用户");
    }
    None
}

pub(super) fn token_refresh_snapshot_for_live_read(
    token_grant: &EncryptedTokenGrantRecord,
) -> TokenRefreshGrantSnapshot {
    TokenRefreshGrantSnapshot {
        grant_id: TokenGrantId(token_grant.id.clone()),
        tenant_id: TenantId(token_grant.tenant_id.clone()),
        expected_fingerprint: token_grant.oauth_grant_fingerprint.clone(),
        state: token_grant.state,
        has_refresh_material: !token_grant.encrypted_oauth_grant.is_empty(),
        revoked_at: token_grant.revoked_at_ms.map(ms_to_system_time),
        reauth_required_at: token_grant.reauth_required_at_ms.map(ms_to_system_time),
    }
}

pub(super) fn ensure_refresh_report_allows_read(
    report: &TokenRefreshServiceReport,
) -> Result<(), LiveGrantRefreshError> {
    match (&report.status, report.command) {
        (
            TokenRefreshReportStatus::Succeeded | TokenRefreshReportStatus::ConflictNoop,
            Some(TokenRefreshCommandKind::RotateGrantCas),
        ) => Ok(()),
        (_, Some(TokenRefreshCommandKind::MarkReauthRequired))
        | (TokenRefreshReportStatus::ShortCircuited(_), _) => {
            Err(LiveGrantRefreshError::ReauthRequired)
        }
        (
            _,
            Some(
                TokenRefreshCommandKind::MarkNeedsRefresh
                | TokenRefreshCommandKind::MarkConfigRequired,
            ),
        )
        | (_, None) => Err(LiveGrantRefreshError::RefreshFailed),
    }
}

pub(super) fn system_time_to_ms(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

fn live_read_refresh_audit_context(
    auth_context: &AuthenticatedContext,
    grant_id: &str,
    occurred_at_ms: u64,
) -> TokenRefreshAuditContext {
    TokenRefreshAuditContext {
        trace_id: safe_live_read_trace_id(auth_context, grant_id, occurred_at_ms),
        sequence: 0,
        occurred_at_ms,
        actor: AuditActor {
            kind: AuditActorKind::User,
            actor_id: auth_context.user_id.clone(),
            display_name: None,
        },
        workspace_id: None,
    }
}

pub(super) fn safe_live_read_trace_id(
    auth_context: &AuthenticatedContext,
    grant_id: &str,
    occurred_at_ms: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(auth_context.tenant_id.as_bytes());
    hasher.update([0]);
    hasher.update(auth_context.user_id.as_bytes());
    hasher.update([0]);
    hasher.update(auth_context.session_id.as_bytes());
    hasher.update([0]);
    hasher.update(grant_id.as_bytes());
    hasher.update([0]);
    hasher.update(occurred_at_ms.to_be_bytes());
    let digest = hex::encode(hasher.finalize());
    format!("live-feishu-read-{}", &digest[..24])
}

fn ms_to_system_time(ms: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_millis(ms)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum LiveGrantRefreshError {
    RefreshUnavailable,
    RefreshFailed,
    ReauthRequired,
    ReloadFailed,
    GrantMissing,
    StillStale,
}

impl LiveGrantRefreshError {
    pub(super) fn safe_reason(self) -> &'static str {
        match self {
            Self::RefreshUnavailable => "授权刷新暂不可用",
            Self::RefreshFailed => "授权令牌刷新失败",
            Self::ReauthRequired => "授权已失效，需要重新登录",
            Self::ReloadFailed => "刷新后读取授权 grant 失败",
            Self::GrantMissing => "刷新后未找到用户授权 grant",
            Self::StillStale => "刷新后授权令牌仍不可用",
        }
    }
}

impl fmt::Debug for LiveGrantRefreshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.safe_reason())
    }
}

#[cfg(test)]
mod tests;

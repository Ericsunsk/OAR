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

pub(super) const TOKEN_REFRESH_SKEW_MS: u64 = 5 * 60 * 1000;
const WORKSPACE_USER_PREFIX: &str = "feishu_user_";
const GRANT_PREFIX: &str = "feishu_grant_";

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

pub(super) async fn resolve_grant_id_for_user(
    pool: &sqlx::PgPool,
    auth_context: &AuthenticatedContext,
) -> Result<String, &'static str> {
    if let Some(user_tail) = auth_context.user_id.strip_prefix(WORKSPACE_USER_PREFIX) {
        let grant_id = format!("{GRANT_PREFIX}{user_tail}");
        let matched_grant_id = sqlx::query_scalar::<_, String>(
            r#"
            SELECT tg.id
            FROM token_grants tg
            INNER JOIN device_sessions ds
              ON ds.tenant_id = tg.tenant_id
             AND ds.user_id = $2
             AND ds.id = $3
            WHERE tg.tenant_id = $1
              AND tg.id = $4
              AND tg.actor_kind = 'user'
              AND tg.scope_boundary = 'user'
              AND tg.revoked_at IS NULL
              AND tg.reauth_required_at IS NULL
              AND ds.state = 'active'
              AND ds.revoked_at IS NULL
              AND ds.expired_at IS NULL
            LIMIT 1
            "#,
        )
        .bind(&auth_context.tenant_id)
        .bind(&auth_context.user_id)
        .bind(&auth_context.session_id)
        .bind(&grant_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| "查询用户授权失败")?;
        return matched_grant_id.ok_or("当前用户没有可用的授权 grant");
    }

    let grant_ids = sqlx::query_scalar::<_, String>(
        r#"
        SELECT tg.id
        FROM token_grants tg
        INNER JOIN lark_identities li
          ON li.tenant_id = tg.tenant_id
         AND li.id = tg.identity_id
        WHERE tg.tenant_id = $1
          AND li.actor_kind = 'user'
          AND li.actor_external_id = $2
          AND tg.revoked_at IS NULL
          AND tg.reauth_required_at IS NULL
        ORDER BY tg.refreshed_at DESC NULLS LAST, tg.issued_at DESC
        LIMIT 2
        "#,
    )
    .bind(&auth_context.tenant_id)
    .bind(&auth_context.user_id)
    .fetch_all(pool)
    .await
    .map_err(|_| "查询用户授权失败")?;

    match grant_ids.as_slice() {
        [] => Err("当前用户没有可用的授权 grant"),
        [grant_id] => Ok(grant_id.clone()),
        _ => Err("用户授权 grant 不唯一"),
    }
}

pub(super) async fn resolve_lark_open_id_for_grant(
    pool: &sqlx::PgPool,
    auth_context: &AuthenticatedContext,
    token_grant: &EncryptedTokenGrantRecord,
) -> Result<String, &'static str> {
    let open_id = sqlx::query_scalar::<_, String>(
        r#"
        SELECT li.actor_external_id
        FROM lark_identities li
        WHERE li.tenant_id = $1
          AND li.id = $2
          AND li.actor_kind = 'user'
        LIMIT 1
        "#,
    )
    .bind(&auth_context.tenant_id)
    .bind(&token_grant.identity_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| "查询用户飞书身份失败")?
    .ok_or("当前用户没有可用的飞书身份")?;

    if open_id.trim().is_empty() {
        return Err("当前用户飞书身份为空");
    }
    Ok(open_id)
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
pub(super) mod tests {
    use super::*;
    use crate::AuthenticatedContext;

    #[test]
    fn live_read_refresh_trace_id_does_not_embed_session_or_grant() {
        let auth_context = AuthenticatedContext {
            session_id: "oar_session_secret".to_string(),
            tenant_id: "tenant_x".to_string(),
            user_id: "feishu_user_secret".to_string(),
        };

        let trace_id = safe_live_read_trace_id(&auth_context, "grant_secret", 42);

        assert!(trace_id.starts_with("live-feishu-read-"));
        assert!(!trace_id.contains("oar_session_secret"));
        assert!(!trace_id.contains("grant_secret"));
        assert!(!trace_id.contains("feishu_user_secret"));
    }
}

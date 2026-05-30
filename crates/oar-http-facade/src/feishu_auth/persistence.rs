use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::time::SystemTime;

use oar_core::domain::device_sync::{DeviceEntryPoint, DeviceSession};
use oar_core::domain::identity::{
    ActorKind, DeviceSessionId, LarkIdentity, LarkIdentityId, ScopeBoundary, Tenant, TenantId,
    TenantStatus, TokenGrantState, WorkspaceUser, WorkspaceUserId, WorkspaceUserStatus,
};
use oar_core::storage::postgres::{
    EncryptedTokenGrantRecord, PostgresDeviceSessionRepository, PostgresLarkIdentityRepository,
    PostgresTenantRepository, PostgresTokenGrantRepository, PostgresWorkspaceUserRepository,
};
use oar_lark_adapter::material::compose_encrypted_grant_blob;
use oar_lark_adapter::{
    AesGcmGrantEncryptor, FeishuGrantEncryptionInput, FeishuGrantEncryptor, FeishuOAuthLogin,
};

use super::util::{stable_prefixed_id, stable_sha256_hex, system_time_to_ms_lossy};
use crate::persistence::FacadePersistenceRuntime;

#[derive(Debug, Clone)]
pub(crate) struct FeishuLoginPersistencePlan {
    pub(crate) tenant: Tenant,
    pub(crate) user: WorkspaceUser,
    pub(crate) identity: LarkIdentity,
    pub(crate) grant: EncryptedTokenGrantRecord,
    pub(crate) session: DeviceSession,
    pub(crate) session_identity_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FeishuLoginPersistenceError {
    MissingTenantKey,
    MissingRefreshToken,
    EncryptGrantFailed,
    StoreFailed { stage: &'static str },
}

impl fmt::Display for FeishuLoginPersistenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingTenantKey => write!(f, "feishu_login_missing_tenant_key"),
            Self::MissingRefreshToken => write!(f, "feishu_login_missing_refresh_token"),
            Self::EncryptGrantFailed => write!(f, "feishu_login_grant_encrypt_failed"),
            Self::StoreFailed { stage } => {
                write!(f, "feishu_login_grant_store_failed:{stage}")
            }
        }
    }
}

impl Error for FeishuLoginPersistenceError {}

pub(super) async fn persist_feishu_login_grant(
    persistence: Option<&FacadePersistenceRuntime>,
    login: &FeishuOAuthLogin,
    oar_session_id: &str,
) -> Result<(), FeishuLoginPersistenceError> {
    let Some(persistence) = persistence else {
        return Ok(());
    };

    let plan = build_feishu_login_persistence_plan(
        login,
        oar_session_id,
        persistence.grant_key_id(),
        persistence.grant_key_material(),
        SystemTime::now(),
    )?;
    PostgresTenantRepository::new(persistence.pool())
        .upsert(&plan.tenant)
        .await
        .map_err(|_| FeishuLoginPersistenceError::StoreFailed { stage: "tenant" })?;
    PostgresWorkspaceUserRepository::new(persistence.pool())
        .upsert(&plan.user)
        .await
        .map_err(|_| FeishuLoginPersistenceError::StoreFailed {
            stage: "workspace_user",
        })?;
    PostgresLarkIdentityRepository::new(persistence.pool())
        .upsert(&plan.identity)
        .await
        .map_err(|_| FeishuLoginPersistenceError::StoreFailed {
            stage: "lark_identity",
        })?;
    PostgresTokenGrantRepository::new(persistence.pool())
        .upsert_encrypted_grant(&plan.grant)
        .await
        .map_err(|_| FeishuLoginPersistenceError::StoreFailed {
            stage: "token_grant",
        })?;
    PostgresDeviceSessionRepository::new(persistence.pool())
        .upsert_with_identity_hash(&plan.session, &plan.session_identity_hash)
        .await
        .map_err(|_| FeishuLoginPersistenceError::StoreFailed {
            stage: "device_session",
        })?;
    Ok(())
}

pub(crate) fn build_feishu_login_persistence_plan(
    login: &FeishuOAuthLogin,
    oar_session_id: &str,
    grant_key_id: &str,
    grant_key_material: [u8; 32],
    now: SystemTime,
) -> Result<FeishuLoginPersistencePlan, FeishuLoginPersistenceError> {
    let tenant_key = login
        .user
        .tenant_key
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or(FeishuLoginPersistenceError::MissingTenantKey)?;
    let refresh_token = login
        .token
        .refresh_token
        .clone()
        .ok_or(FeishuLoginPersistenceError::MissingRefreshToken)?;

    let tenant_id = TenantId(stable_prefixed_id("feishu_tenant", &[tenant_key]));
    let user_id = WorkspaceUserId(stable_prefixed_id(
        "feishu_user",
        &[tenant_key, &login.user.open_id],
    ));
    let identity_id = LarkIdentityId(stable_prefixed_id(
        "feishu_identity",
        &[tenant_key, &login.user.open_id],
    ));
    let grant_id = stable_prefixed_id("feishu_grant", &[tenant_key, &login.user.open_id]);
    let mut encryptor = AesGcmGrantEncryptor::new(grant_key_id.to_string(), grant_key_material);
    let envelope = encryptor
        .encrypt(FeishuGrantEncryptionInput {
            grant_id: grant_id.clone(),
            tenant_id: tenant_id.0.clone(),
            expected_fingerprint: "initial_login".to_string(),
            access_token: login.token.access_token.clone(),
            refresh_token,
            expires_in_seconds: login.token.expires_in_seconds,
            refresh_token_expires_in_seconds: login.token.refresh_token_expires_in_seconds,
            token_type: login.token.token_type.clone(),
            scope: login.token.scope.clone(),
        })
        .map_err(|_| FeishuLoginPersistenceError::EncryptGrantFailed)?;
    let encrypted_oauth_grant =
        compose_encrypted_grant_blob(envelope.encrypted_primary, envelope.encrypted_renewal);
    let issued_at_ms = system_time_to_ms_lossy(now);
    let grant = EncryptedTokenGrantRecord {
        id: grant_id,
        tenant_id: tenant_id.0.clone(),
        identity_id: identity_id.0.clone(),
        actor_kind: ActorKind::User,
        scope_boundary: ScopeBoundary::User,
        scopes: oauth_scope_list(login.token.scope.as_deref()),
        state: TokenGrantState::Valid,
        issued_at_ms,
        expires_at_ms: envelope.expires_at_ms,
        refreshed_at_ms: Some(envelope.refreshed_at_ms),
        revoked_at_ms: None,
        reauth_required_at_ms: None,
        last_refresh_error: None,
        encrypted_oauth_grant,
        oauth_grant_key_id: envelope.key_id,
        oauth_grant_fingerprint: envelope.new_fingerprint,
        revocation_reason: None,
    };
    let session = DeviceSession::new(
        DeviceSessionId(oar_session_id.to_string()),
        tenant_id.clone(),
        user_id.clone(),
        DeviceEntryPoint::MacOs,
        "review_inbox",
        0,
        now,
    );
    let session_identity_hash = stable_sha256_hex(&[&tenant_id.0, &user_id.0, oar_session_id]);

    Ok(FeishuLoginPersistencePlan {
        tenant: Tenant {
            id: tenant_id.clone(),
            display_name: tenant_key.to_string(),
            status: TenantStatus::Active,
        },
        user: WorkspaceUser {
            id: user_id,
            tenant_id: tenant_id.clone(),
            display_name: login.user.display_name.clone(),
            status: WorkspaceUserStatus::Active,
        },
        identity: LarkIdentity {
            id: identity_id,
            tenant_id,
            actor_kind: ActorKind::User,
            actor_external_id: login.user.open_id.clone(),
            display_name: Some(login.user.display_name.clone()),
        },
        grant,
        session,
        session_identity_hash,
    })
}

fn oauth_scope_list(scope: Option<&str>) -> Vec<String> {
    scope
        .into_iter()
        .flat_map(str::split_whitespace)
        .filter(|scope| !scope.trim().is_empty())
        .map(ToOwned::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

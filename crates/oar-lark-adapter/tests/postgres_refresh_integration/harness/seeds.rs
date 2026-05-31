use oar_core::action::audit_event::{AuditActor, AuditActorKind};
use oar_core::action::token_refresh_audit::TokenRefreshAuditContext;
use oar_core::domain::identity::{ActorKind, ScopeBoundary, TokenGrantState};
use oar_core::storage::postgres::{EncryptedTokenGrantRecord, PostgresTokenGrantRepository};
use sqlx::PgPool;

use super::constants::{ACTOR_ID, IDENTITY_ID, KEY_ID, OLD_FP, TENANT_ID, USER_ID};

pub(crate) async fn seed_refresh_candidate_grant(
    pool: &PgPool,
    grant_id: &str,
    blob: Vec<u8>,
) -> Result<(), oar_core::storage::postgres::PostgresRepositoryError> {
    seed_refresh_candidate_grant_with_key_id_and_scopes(
        pool,
        grant_id,
        KEY_ID,
        default_refresh_scopes(),
        blob,
    )
    .await
}

pub(crate) async fn seed_refresh_candidate_grant_with_key_id_and_scopes(
    pool: &PgPool,
    grant_id: &str,
    key_id: &str,
    scopes: Vec<String>,
    blob: Vec<u8>,
) -> Result<(), oar_core::storage::postgres::PostgresRepositoryError> {
    PostgresTokenGrantRepository::new(pool.clone())
        .upsert_encrypted_grant(&EncryptedTokenGrantRecord {
            id: grant_id.to_string(),
            tenant_id: TENANT_ID.to_string(),
            identity_id: IDENTITY_ID.to_string(),
            actor_kind: ActorKind::User,
            scope_boundary: ScopeBoundary::User,
            scopes,
            state: TokenGrantState::NeedsRefresh,
            issued_at_ms: 1_779_460_000_000,
            expires_at_ms: Some(1_779_465_500_000),
            refreshed_at_ms: Some(1_779_465_000_000),
            revoked_at_ms: None,
            reauth_required_at_ms: None,
            last_refresh_error: Some("old-error".to_string()),
            encrypted_oauth_grant: blob,
            oauth_grant_key_id: key_id.to_string(),
            oauth_grant_fingerprint: OLD_FP.to_string(),
            revocation_reason: None,
        })
        .await?;
    Ok(())
}

fn default_refresh_scopes() -> Vec<String> {
    vec![
        "offline_access".to_string(),
        "auth:user.id:read".to_string(),
        "okr.progress.write".to_string(),
    ]
}

pub(crate) fn audit_context(trace_id: &str, sequence: u64) -> TokenRefreshAuditContext {
    TokenRefreshAuditContext {
        trace_id: trace_id.to_string(),
        sequence,
        occurred_at_ms: 1_779_466_000_123,
        actor: AuditActor {
            kind: AuditActorKind::User,
            actor_id: ACTOR_ID.to_string(),
            display_name: Some("Reviewer".to_string()),
        },
        workspace_id: None,
    }
}

pub(crate) async fn seed_identity_graph(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO tenants (id, display_name, status)
        VALUES ($1, $2, 'active')
        "#,
    )
    .bind(TENANT_ID)
    .bind("Adapter integration tenant")
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO workspace_users (id, tenant_id, display_name, status)
        VALUES ($1, $2, $3, 'active')
        "#,
    )
    .bind(USER_ID)
    .bind(TENANT_ID)
    .bind("Adapter integration user")
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO lark_identities (id, tenant_id, actor_kind, actor_external_id, display_name)
        VALUES ($1, $2, 'user', $3, $4)
        "#,
    )
    .bind(IDENTITY_ID)
    .bind(TENANT_ID)
    .bind("ext_adapter_pg_refresh")
    .bind("Adapter integration identity")
    .execute(pool)
    .await?;

    Ok(())
}

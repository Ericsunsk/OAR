use std::time::SystemTime;

use oar_core::action::capability::all_capabilities;
use oar_core::action::confirmed_action::ConfirmedAction;
use oar_core::action::execution_policy::{ActionActorBinding, ExecutionPolicy};
use oar_core::action::execution_request::{ConfirmedExecutionDecision, ConfirmedExecutionRequest};
use oar_core::action::postgres_executor::PostgresActionExecutor;
use oar_core::domain::identity::{
    ActorKind, LarkIdentityId, OAuthTokens, ScopeBoundary, SecretString, TenantId, TokenGrant,
    TokenGrantId, TokenGrantState,
};
use oar_core::domain::proposed_action::ProposedActionKind;
use oar_core::storage::postgres::{PostgresAuditEventRepository, PostgresExecutionRecorder};
use serde_json::json;
use sqlx::PgPool;

use super::LiveMockAdapter;

pub(crate) fn confirmed_action(
    action_id: &str,
    tenant_id: &str,
    actor_user_id: &str,
    idempotency_key: &str,
) -> ConfirmedAction {
    ConfirmedAction::proposed(action_id, tenant_id, actor_user_id, idempotency_key)
        .confirm(SystemTime::UNIX_EPOCH)
}

pub(crate) fn confirmed_execution_request(action: ConfirmedAction) -> ConfirmedExecutionRequest {
    ConfirmedExecutionRequest {
        proposed_action_id: action.action_id.clone(),
        proposed_action_version: 1,
        action_kind: ProposedActionKind::UpdateKrProgress,
        target_user_id: Some(action.actor_user_id.clone()),
        owner_user_id: None,
        evidence_ids: vec!["evidence_1".to_string()],
        effective_payload: json!({
            "target": {
                "objective_id": "objective_live_alpha",
                "kr_id": "kr_live_beta"
            },
            "mutation": {
                "progress_delta": 1,
                "note": "live executor test"
            }
        }),
        decision: ConfirmedExecutionDecision::Confirm,
        confirmed_action: action,
    }
}

pub(crate) fn postgres_action_executor(
    pool: PgPool,
    adapter: LiveMockAdapter,
) -> PostgresActionExecutor<LiveMockAdapter, impl FnMut() -> u64> {
    let mut tick = 1_748_260_000_000_u64;
    PostgresActionExecutor::new(
        adapter,
        move || {
            tick += 1_000;
            tick
        },
        PostgresExecutionRecorder::new(pool.clone()),
        PostgresAuditEventRepository::new(pool),
    )
}

pub(crate) fn token_grant(tenant_id: &str, scopes: &[&str], state: TokenGrantState) -> TokenGrant {
    TokenGrant {
        id: TokenGrantId("grant_live".to_string()),
        tenant_id: TenantId(tenant_id.to_string()),
        identity_id: LarkIdentityId("identity_live".to_string()),
        actor_kind: ActorKind::User,
        scope_boundary: ScopeBoundary::User,
        scopes: scopes.iter().map(|scope| (*scope).to_string()).collect(),
        state,
        issued_at: SystemTime::UNIX_EPOCH,
        expires_at: None,
        refreshed_at: None,
        revoked_at: None,
        reauth_required_at: None,
        last_refresh_error: None,
        tokens: OAuthTokens {
            access_token: SecretString::new("access-token"),
            refresh_token: Some(SecretString::new("refresh-token")),
        },
        revocation_reason: None,
    }
}

pub(crate) fn actor_binding(actor_user_id: &str) -> ActionActorBinding {
    ActionActorBinding::new(actor_user_id, LarkIdentityId("identity_live".to_string()))
}

pub(crate) fn okr_progress_write_policy() -> ExecutionPolicy {
    ExecutionPolicy::from_capabilities(all_capabilities(), [ActorKind::User, ActorKind::Service])
}

pub(crate) async fn audit_outbox_count(pool: &PgPool, tenant_id: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM audit_outbox
        WHERE tenant_id = $1
        "#,
    )
    .bind(tenant_id)
    .fetch_one(pool)
    .await
}

#![cfg(feature = "postgres")]

pub(crate) use std::sync::{Arc, Mutex};
pub(crate) use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) use oar_core::action::audit_event::{
    AuditActor, AuditActorKind, AuditEvent, AuditEventContext, AuditEventType, AuditScope,
    AuditStateSummary, AuditSubject, AuditTarget,
};
pub(crate) use oar_core::action::capability::all_capabilities;
pub(crate) use oar_core::action::confirmed_action::{ActionStatus, ConfirmedAction};
pub(crate) use oar_core::action::execution_policy::{
    ActionActorBinding, ExecutionDenied, ExecutionPolicy,
};
pub(crate) use oar_core::action::execution_request::{
    ConfirmedExecutionDecision, ConfirmedExecutionRequest,
};
pub(crate) use oar_core::action::executor::{
    ActionAdapter, AdapterDryRun, AdapterError, AdapterExecution, ExecutionError,
};
pub(crate) use oar_core::action::operation_ledger::{LedgerError, SubmitResult};
pub(crate) use oar_core::action::postgres_executor::PostgresActionExecutor;
pub(crate) use oar_core::action::token_refresh_audit::{
    token_refresh_audit_event, TokenRefreshAuditContext,
};
pub(crate) use oar_core::domain::device_sync::{DeviceEntryPoint, DeviceSession, SessionState};
pub(crate) use oar_core::domain::evidence::{
    EvidenceId, EvidenceItem, EvidenceRef, EvidenceSourceKind, EvidenceVisibilityScope,
};
pub(crate) use oar_core::domain::identity::{
    ActorKind, DeviceSessionId, LarkIdentity, LarkIdentityId, OAuthTokens, ScopeBoundary,
    SecretString, Tenant, TenantId, TenantStatus, TokenGrant, TokenGrantId, TokenGrantState,
    WorkspaceUser, WorkspaceUserId, WorkspaceUserStatus,
};
pub(crate) use oar_core::domain::proposed_action::{
    ProposedAction, ProposedActionDecision, ProposedActionId, ProposedActionKind, RiskSeverity,
};
pub(crate) use oar_core::domain::review_inbox::{
    ReviewInboxItem, ReviewInboxItemId, ReviewInboxItemStatus,
};
pub(crate) use oar_core::domain::scheduler::{
    SchedulerJobKind, SchedulerJobOutcome, SchedulerJobStatus, SchedulerLeaseAcquire,
};
pub(crate) use oar_core::domain::token_refresh::types::{
    EncryptedGrantBlob, EncryptedGrantMaterial, RefreshOutcome, TokenRefreshAuditSummary,
    TokenRefreshCommandKind, TokenRefreshCommandReport, TokenRefreshDecisionKind,
    TokenRefreshGrantSnapshot, TokenRefreshPlannedCommand, TokenRefreshReportStatus,
    TokenRefreshRepositoryCommand,
};
pub(crate) use oar_core::lark::auth::adapter::FeishuAuthRefreshAdapter;
pub(crate) use oar_core::lark::fixtures::{
    AUTH_REFRESH_PLAINTEXT_TOKEN_LEAK_JSON, AUTH_REFRESH_REAUTH_REQUIRED_JSON,
    AUTH_REFRESH_ROTATED_ENCRYPTED_JSON,
};
pub(crate) use oar_core::storage::postgres::audit_outbox_worker::{
    AuditOutboxDelivery, AuditOutboxDispatcher, AuditOutboxDrainConfig, PostgresAuditOutboxWorker,
};
pub(crate) use oar_core::storage::postgres::tenant_maintenance::{
    PostgresTenantMaintenanceConfig, PostgresTenantMaintenanceWorker,
};
pub(crate) use oar_core::storage::postgres::{
    AuditOutboxEnvelope, AuditOutboxMessage, EncryptedTokenGrantRecord,
    InsertProposedActionDecisionRequest, PostgresAuditEventRepository,
    PostgresDeviceSessionRepository, PostgresExecutionRecorder, PostgresLarkIdentityRepository,
    PostgresOperationLedgerRepository, PostgresRepositoryError,
    PostgresReviewDecisionContextRequest, PostgresReviewDecisionRecorder,
    PostgresReviewDecisionRecorderRequest, PostgresReviewInboxRepository,
    PostgresSchedulerJobRepository, PostgresTenantRepository, PostgresTokenGrantRepository,
    PostgresTokenRefreshOrchestrator, PostgresTokenRefreshRecorder,
    PostgresTokenRefreshScheduledSweep, PostgresTokenRefreshSweep,
    PostgresTokenRefreshSweepRequest, PostgresWorkspaceUserRepository, RotateEncryptedGrantRequest,
    TokenRefreshScheduledSweepConfig,
};
pub(crate) use serde_json::json;
pub(crate) use sqlx::postgres::PgPoolOptions;
pub(crate) use sqlx::{PgPool, Row};

#[path = "harness/live_db.rs"]
mod live_db;
#[path = "harness/test_doubles.rs"]
mod test_doubles;
#[path = "harness/token_refresh_fixtures.rs"]
mod token_refresh_fixtures;

pub(crate) use live_db::{run_live_postgres_test, runtime};
pub(crate) use test_doubles::{
    FixtureClient, LiveMockAdapter, LiveOutboxDispatcher, LiveRefreshAdapter,
    SequenceRefreshAdapter,
};
pub(crate) use token_refresh_fixtures::{
    assert_no_auth_refresh_sensitive_payload, encrypted_token_grant_record,
    planned_token_refresh_command, rotate_grant_request,
};

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

pub(crate) fn actor(actor_id: &str) -> AuditActor {
    AuditActor {
        kind: AuditActorKind::User,
        actor_id: actor_id.to_string(),
        display_name: Some("Reviewer".to_string()),
    }
}

pub(crate) fn scope(tenant_id: &str) -> AuditScope {
    AuditScope {
        tenant_id: tenant_id.to_string(),
        workspace_id: None,
    }
}

pub(crate) fn target(resource_id: &str) -> AuditTarget {
    AuditTarget {
        resource_type: "okr_progress".to_string(),
        resource_id: resource_id.to_string(),
        action_type: "update_progress".to_string(),
    }
}

pub(crate) fn summary(text: &str) -> AuditStateSummary {
    AuditStateSummary {
        summary: text.to_string(),
        reference_ids: vec!["evidence_1".to_string()],
        content_hash: Some("sha256:abc123".to_string()),
    }
}

pub(crate) fn audit_context(
    event_id: &str,
    trace_id: &str,
    sequence: u64,
    occurred_at_ms: u64,
    actor_id: &str,
    tenant_id: &str,
    resource_id: &str,
) -> AuditEventContext {
    AuditEventContext {
        event_id: event_id.to_string(),
        trace_id: trace_id.to_string(),
        sequence,
        occurred_at_ms,
        subject: AuditSubject {
            actor: actor(actor_id),
            scope: scope(tenant_id),
            target: target(resource_id),
        },
    }
}

pub(crate) fn outbox_envelope(
    tenant_id: &str,
    trace_id: &str,
    next_attempt_at_ms: u64,
) -> AuditOutboxEnvelope {
    AuditOutboxEnvelope {
        tenant_id: tenant_id.to_string(),
        stream: "audit-events".to_string(),
        aggregate_id: trace_id.to_string(),
        payload: json!({ "trace_id": trace_id }),
        next_attempt_at_ms,
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

pub(crate) async fn seed_user(
    pool: &PgPool,
    tenant_id: &str,
    user_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO tenants (id, display_name, status)
        VALUES ($1, $2, 'active')
        "#,
    )
    .bind(tenant_id)
    .bind(format!("Tenant {tenant_id}"))
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO workspace_users (id, tenant_id, display_name, status)
        VALUES ($1, $2, $3, 'active')
        "#,
    )
    .bind(user_id)
    .bind(tenant_id)
    .bind(format!("User {user_id}"))
    .execute(pool)
    .await?;

    Ok(())
}

pub(crate) async fn seed_identity(
    pool: &PgPool,
    tenant_id: &str,
    identity_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO lark_identities (id, tenant_id, actor_kind, actor_external_id, display_name)
        VALUES ($1, $2, 'user', $3, $4)
        "#,
    )
    .bind(identity_id)
    .bind(tenant_id)
    .bind(format!("ext_{identity_id}"))
    .bind(format!("Identity {identity_id}"))
    .execute(pool)
    .await?;

    Ok(())
}

pub(crate) fn device_session(
    tenant_id: &str,
    user_id: &str,
    session_id: &str,
    stream: &str,
    cursor: u64,
    now: SystemTime,
) -> DeviceSession {
    DeviceSession::new(
        DeviceSessionId(session_id.to_string()),
        TenantId(tenant_id.to_string()),
        WorkspaceUserId(user_id.to_string()),
        DeviceEntryPoint::MacOs,
        stream.to_string(),
        cursor,
        now,
    )
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

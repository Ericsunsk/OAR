#![cfg(feature = "postgres")]

pub(crate) use std::sync::{Arc, Mutex};
pub(crate) use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) use oar_core::action::audit_event::{
    AuditActor, AuditActorKind, AuditEvent, AuditEventContext, AuditEventType, AuditScope,
    AuditStateSummary, AuditSubject, AuditTarget,
};
pub(crate) use oar_core::action::confirmed_action::{ActionStatus, ConfirmedAction};
pub(crate) use oar_core::action::execution_policy::ExecutionDenied;
pub(crate) use oar_core::action::execution_request::ConfirmedExecutionRequest;
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
    ActorKind, DeviceSessionId, LarkIdentity, LarkIdentityId, ScopeBoundary, Tenant, TenantId,
    TenantStatus, TokenGrantId, TokenGrantState, WorkspaceUser, WorkspaceUserId,
    WorkspaceUserStatus,
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
    TokenRefreshScheduledSweepConfig, TOKEN_REFRESH_SWEEP_SCHEDULER_JOB_ID,
};
pub(crate) use serde_json::json;
pub(crate) use sqlx::postgres::PgPoolOptions;
pub(crate) use sqlx::{PgPool, Row};

#[path = "harness/action_fixtures.rs"]
mod action_fixtures;
#[path = "harness/audit_fixtures.rs"]
mod audit_fixtures;
#[path = "harness/live_db.rs"]
mod live_db;
#[path = "harness/test_doubles.rs"]
mod test_doubles;
#[path = "harness/token_refresh_fixtures.rs"]
mod token_refresh_fixtures;

pub(crate) use action_fixtures::{
    actor_binding, audit_outbox_count, confirmed_action, confirmed_execution_request,
    okr_progress_write_policy, postgres_action_executor, token_grant,
};
pub(crate) use audit_fixtures::{actor, audit_context, outbox_envelope, scope, summary};
pub(crate) use live_db::{run_live_postgres_test, runtime};
pub(crate) use test_doubles::{
    FixtureClient, LiveMockAdapter, LiveOutboxDispatcher, LiveRefreshAdapter,
    SequenceRefreshAdapter,
};
pub(crate) use token_refresh_fixtures::{
    assert_no_auth_refresh_sensitive_payload, encrypted_token_grant_record,
    planned_token_refresh_command, rotate_grant_request,
};

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

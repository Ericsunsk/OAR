use super::*;
use oar_core::action::audit_event::AuditEvent;
use oar_core::action::confirmed_action::ConfirmedAction;
use oar_core::action::postgres_executor::PostgresActionExecutor;
use oar_core::action::token_refresh_audit::{token_refresh_audit_event, TokenRefreshAuditContext};
use oar_core::domain::identity::{ActorKind, ScopeBoundary, TokenGrantState};
use oar_core::domain::scheduler::{
    SchedulerJobKind, SchedulerJobLease, SchedulerJobStatus, SchedulerLeaseAcquire,
};
use oar_core::domain::token_refresh::service::{AsyncAuthRefreshAdapter, AuthRefreshAdapter};
use oar_core::domain::token_refresh::types::{RefreshOutcome, TokenRefreshGrantSnapshot};
use oar_core::lark::adapter::MockLarkAdapter;
use oar_core::storage::postgres::audit_outbox_worker::{
    AuditOutboxDelivery, AuditOutboxDispatcher, AuditOutboxDrainConfig, AuditOutboxDrainReport,
    PostgresAuditOutboxWorker,
};
use oar_core::storage::postgres::tenant_maintenance::{
    PostgresTenantMaintenanceConfig, PostgresTenantMaintenanceConfigValidationError,
    PostgresTenantMaintenanceReport, PostgresTenantMaintenanceWorker,
};
use oar_core::storage::postgres::{
    AuditOutboxEnvelope, EncryptedTokenGrantRecord, PostgresAuditEventRepository,
    PostgresAuthLifecycleRepository, PostgresAuthLogoutRevokeRequest,
    PostgresDeviceSessionRepository, PostgresExecutionRecorder, PostgresExecutionRecorderReport,
    PostgresIdentityRepository, PostgresLarkIdentityRepository, PostgresOperationLedgerRepository,
    PostgresReviewDecisionContextRequest, PostgresReviewDecisionRecorder,
    PostgresReviewDecisionRecorderReport, PostgresReviewDecisionRecorderRequest,
    PostgresReviewInboxRepository, PostgresSchedulerJobRepository, PostgresTenantRepository,
    PostgresTokenGrantRepository, PostgresTokenRefreshOrchestrator, PostgresTokenRefreshRecorder,
    PostgresTokenRefreshScheduledSweep, PostgresTokenRefreshSweep, PostgresTokenRefreshSweepReport,
    PostgresTokenRefreshSweepRequest, PostgresWorkspaceUserRepository, RotateEncryptedGrantRequest,
    StoredDeviceSession, StoredLarkIdentity, StoredReviewDecisionContext,
    StoredReviewInboxLedgerEvent, StoredReviewInboxLedgerStage, StoredReviewInboxLedgerStatus,
    StoredSchedulerJob, StoredTenant, StoredWorkspaceUser, TokenRefreshScheduledSweepConfig,
    TokenRefreshScheduledSweepReport,
};
use sqlx::PgPool;

type NoopTenantMaintenanceWorker =
    PostgresTenantMaintenanceWorker<NoopRefreshAdapter, NoopDispatcher, fn() -> u64>;

#[test]
fn postgres_repositories_are_importable_and_constructible_from_pg_pool() {
    let _from_pool_ctor_op: fn(PgPool) -> PostgresOperationLedgerRepository =
        PostgresOperationLedgerRepository::new;
    let _from_pool_ctor_audit: fn(PgPool) -> PostgresAuditEventRepository =
        PostgresAuditEventRepository::new;
    let _from_pool_ctor_recorder: fn(PgPool) -> PostgresExecutionRecorder =
        PostgresExecutionRecorder::new;
    let _from_pool_ctor_review_decision_recorder: fn(PgPool) -> PostgresReviewDecisionRecorder =
        PostgresReviewDecisionRecorder::new;
    let _from_pool_ctor_token_refresh_recorder: fn(PgPool) -> PostgresTokenRefreshRecorder =
        PostgresTokenRefreshRecorder::new;
    let _from_pool_ctor_auth_lifecycle: fn(PgPool) -> PostgresAuthLifecycleRepository =
        PostgresAuthLifecycleRepository::new;
    let _from_pool_ctor_token_grant: fn(PgPool) -> PostgresTokenGrantRepository =
        PostgresTokenGrantRepository::new;
    let _from_pool_ctor_device_session: fn(PgPool) -> PostgresDeviceSessionRepository =
        PostgresDeviceSessionRepository::new;
    let _from_pool_ctor_tenant: fn(PgPool) -> PostgresTenantRepository =
        PostgresTenantRepository::new;
    let _from_pool_ctor_workspace_user: fn(PgPool) -> PostgresWorkspaceUserRepository =
        PostgresWorkspaceUserRepository::new;
    let _from_pool_ctor_lark_identity: fn(PgPool) -> PostgresLarkIdentityRepository =
        PostgresLarkIdentityRepository::new;
    let _from_pool_ctor_identity_repo: fn(PgPool) -> PostgresIdentityRepository =
        PostgresIdentityRepository::new;
    let _from_pool_ctor_scheduler: fn(PgPool) -> PostgresSchedulerJobRepository =
        PostgresSchedulerJobRepository::new;

    // Keep SQL constants reachable under the feature build too.
    let _ = compact(SUBMIT_CONFIRMED_ACTION_AND_LEDGER);
}

#[test]
fn postgres_repository_async_methods_are_type_checked() {
    let _submit = PostgresOperationLedgerRepository::submit_confirmed_action;
    let _mark_executing = PostgresOperationLedgerRepository::mark_executing;
    let _mark_succeeded = PostgresOperationLedgerRepository::mark_succeeded;
    let _mark_failed = PostgresOperationLedgerRepository::mark_failed;
    let _get = PostgresOperationLedgerRepository::get_by_idempotency_key;
    let _append = PostgresAuditEventRepository::append;
    let _find = PostgresAuditEventRepository::find_by_tenant_and_trace_id;
    let _enqueue = PostgresAuditEventRepository::enqueue_outbox;
    let _claim = PostgresAuditEventRepository::claim_outbox;
    let _sent = PostgresAuditEventRepository::mark_outbox_sent;
    let _sent_for_attempt = PostgresAuditEventRepository::mark_outbox_sent_for_attempt;
    let _retryable = PostgresAuditEventRepository::mark_outbox_retryable;
    let _retryable_for_attempt = PostgresAuditEventRepository::mark_outbox_retryable_for_attempt;
    let _failed = PostgresAuditEventRepository::mark_outbox_failed;
    let _failed_for_attempt = PostgresAuditEventRepository::mark_outbox_failed_for_attempt;
    let _token_refresh_mapping = token_refresh_audit_event;
    let _token_refresh_append = PostgresAuditEventRepository::append;
    let _token_refresh_audit_pipeline = (_token_refresh_mapping, _token_refresh_append);
    let _record_confirmation = PostgresExecutionRecorder::record_confirmation;
    let _record_dry_run = PostgresExecutionRecorder::record_dry_run;
    let _record_success = PostgresExecutionRecorder::record_success;
    let _record_failure = PostgresExecutionRecorder::record_failure;
    let _load_review_decision_context =
        PostgresReviewDecisionRecorder::load_review_decision_context;
    let _load_review_decision_context_from_inbox =
        PostgresReviewInboxRepository::load_review_decision_context;
    let _record_review_decision = PostgresReviewDecisionRecorder::record_decision;
    let _execute =
        PostgresActionExecutor::<MockLarkAdapter, fn() -> u64>::execute_confirmed_request;
    let _execute_with_policy =
        PostgresActionExecutor::<MockLarkAdapter, fn() -> u64>::execute_confirmed_request_with_policy;
    let _upsert_grant = PostgresTokenGrantRepository::upsert_encrypted_grant;
    let _get_grant = PostgresTokenGrantRepository::get_by_id;
    let _apply_refresh_command = PostgresTokenGrantRepository::apply_refresh_command;
    let _apply_planned_refresh_command_with_audit =
        PostgresTokenRefreshRecorder::apply_planned_command_with_audit;
    let _auth_logout_revoke =
        PostgresAuthLifecycleRepository::revoke_logout_session_and_last_device_grants;
    let _token_refresh_orchestrator_ctor =
        PostgresTokenRefreshOrchestrator::<NoopRefreshAdapter>::new;
    let _token_refresh_orchestrator_refresh =
        PostgresTokenRefreshOrchestrator::<NoopRefreshAdapter>::refresh_grant_with_audit;
    let _token_refresh_sweep_ctor = PostgresTokenRefreshSweep::<NoopRefreshAdapter>::new;
    let _token_refresh_sweep_run_once =
        PostgresTokenRefreshSweep::<NoopRefreshAdapter>::run_once_for_tenant;
    let _token_refresh_scheduled_sweep_ctor =
        PostgresTokenRefreshScheduledSweep::<NoopRefreshAdapter, fn() -> u64>::new;
    let _token_refresh_scheduled_sweep_run_once =
        PostgresTokenRefreshScheduledSweep::<NoopRefreshAdapter, fn() -> u64>::run_once;
    let _tenant_maintenance_ctor =
        PostgresTenantMaintenanceWorker::<NoopRefreshAdapter, NoopDispatcher, fn() -> u64>::new;
    let _tenant_maintenance_try_ctor =
        PostgresTenantMaintenanceWorker::<NoopRefreshAdapter, NoopDispatcher, fn() -> u64>::try_new;
    let _tenant_maintenance_run_once = PostgresTenantMaintenanceWorker::<
        NoopRefreshAdapter,
        NoopDispatcher,
        fn() -> u64,
    >::run_once;
    let _scheduler_upsert = PostgresSchedulerJobRepository::upsert_job;
    let _scheduler_insert_if_missing = PostgresSchedulerJobRepository::insert_job_if_missing;
    let _scheduler_get = PostgresSchedulerJobRepository::get_job;
    let _scheduler_try_acquire = PostgresSchedulerJobRepository::try_acquire;
    let _scheduler_complete = PostgresSchedulerJobRepository::complete_for_lease;
    let _scheduler_fail = PostgresSchedulerJobRepository::fail_for_lease;
    let _rotate_grant = PostgresTokenGrantRepository::rotate_encrypted_grant;
    let _mark_refresh_failed = PostgresTokenGrantRepository::mark_refresh_failed;
    let _mark_reauth_required = PostgresTokenGrantRepository::mark_reauth_required;
    let _revoke_grant = PostgresTokenGrantRepository::revoke;
    let _list_refresh_candidates = PostgresTokenGrantRepository::list_refresh_candidate_snapshots;
    let _upsert_session = PostgresDeviceSessionRepository::upsert_with_identity_hash;
    let _get_session = PostgresDeviceSessionRepository::get_by_id;
    let _advance_session = PostgresDeviceSessionRepository::advance_cursor_cas;
    let _revoke_session = PostgresDeviceSessionRepository::revoke;
    let _expire_session = PostgresDeviceSessionRepository::expire;
    let _upsert_tenant = PostgresTenantRepository::upsert;
    let _get_tenant = PostgresTenantRepository::get_by_id;
    let _upsert_workspace_user = PostgresWorkspaceUserRepository::upsert;
    let _get_workspace_user = PostgresWorkspaceUserRepository::get_by_id;
    let _upsert_lark_identity = PostgresLarkIdentityRepository::upsert;
    let _get_lark_identity = PostgresLarkIdentityRepository::get_by_id;
    let _get_lark_identity_external = PostgresLarkIdentityRepository::get_by_actor_external_id;
    let _tenant_subrepo = PostgresIdentityRepository::tenants;
    let _user_subrepo = PostgresIdentityRepository::users;
    let _identity_subrepo = PostgresIdentityRepository::identities;
    let _phantom_action: Option<ConfirmedAction> = None;
    let _phantom_event: Option<AuditEvent> = None;
    let _phantom_envelope: Option<AuditOutboxEnvelope> = None;
    let _phantom_report: Option<PostgresExecutionRecorderReport> = None;
    let _phantom_review_decision_report: Option<PostgresReviewDecisionRecorderReport> = None;
    let _phantom_review_decision_request: Option<PostgresReviewDecisionRecorderRequest<'static>> =
        None;
    let _phantom_review_decision_context_request: Option<
        PostgresReviewDecisionContextRequest<'static>,
    > = None;
    let _phantom_review_decision_context: Option<StoredReviewDecisionContext> = None;
    let _phantom_review_inbox_ledger_event: Option<StoredReviewInboxLedgerEvent> = None;
    let _phantom_review_inbox_ledger_stage: Option<StoredReviewInboxLedgerStage> = None;
    let _phantom_review_inbox_ledger_status: Option<StoredReviewInboxLedgerStatus> = None;
    let _phantom_delivery: Option<AuditOutboxDelivery> = None;
    let _phantom_drain_report: Option<AuditOutboxDrainReport> = None;
    let _phantom_config: Option<AuditOutboxDrainConfig> = None;
    let _phantom_worker: Option<PostgresAuditOutboxWorker<NoopDispatcher, fn() -> u64>> = None;
    let _phantom_maintenance_worker: Option<NoopTenantMaintenanceWorker> = None;
    let _phantom_session: Option<StoredDeviceSession> = None;
    let _phantom_tenant: Option<StoredTenant> = None;
    let _phantom_workspace_user: Option<StoredWorkspaceUser> = None;
    let _phantom_lark_identity: Option<StoredLarkIdentity> = None;
    let _phantom_token_refresh_context: Option<TokenRefreshAuditContext> = None;
    let _phantom_auth_logout_revoke: Option<PostgresAuthLogoutRevokeRequest<'static>> = None;
    let _phantom_token_refresh_sweep_request: Option<PostgresTokenRefreshSweepRequest> = None;
    let _phantom_token_refresh_sweep_report: Option<PostgresTokenRefreshSweepReport> = None;
    let _phantom_rotate_request: Option<RotateEncryptedGrantRequest<'static>> = None;
    let _phantom_scheduler_job: Option<StoredSchedulerJob> = None;
    let _phantom_scheduler_kind: Option<SchedulerJobKind> = None;
    let _phantom_scheduler_status: Option<SchedulerJobStatus> = None;
    let _phantom_scheduler_lease: Option<SchedulerJobLease> = None;
    let _phantom_scheduler_acquire: Option<SchedulerLeaseAcquire> = None;
    let _phantom_scheduled_sweep_config: Option<TokenRefreshScheduledSweepConfig> = None;
    let _phantom_scheduled_sweep_report: Option<TokenRefreshScheduledSweepReport> = None;
    let _phantom_tenant_maintenance_config: Option<PostgresTenantMaintenanceConfig> = None;
    let _phantom_tenant_maintenance_config_validation_error: Option<
        PostgresTenantMaintenanceConfigValidationError,
    > = None;
    let _phantom_tenant_maintenance_report: Option<PostgresTenantMaintenanceReport> = None;
    let _phantom_grant = Some(EncryptedTokenGrantRecord {
        id: "grant".to_string(),
        tenant_id: "tenant".to_string(),
        identity_id: "identity".to_string(),
        actor_kind: ActorKind::User,
        scope_boundary: ScopeBoundary::User,
        scopes: vec!["offline_access".to_string()],
        state: TokenGrantState::Valid,
        issued_at_ms: 1,
        expires_at_ms: Some(2),
        refreshed_at_ms: None,
        revoked_at_ms: None,
        reauth_required_at_ms: None,
        last_refresh_error: None,
        encrypted_oauth_grant: vec![1, 2, 3],
        oauth_grant_key_id: "key".to_string(),
        oauth_grant_fingerprint: "fingerprint".to_string(),
        revocation_reason: None,
    });
}

struct NoopDispatcher;
struct NoopRefreshAdapter;

impl AuditOutboxDispatcher for NoopDispatcher {
    type Error = ();

    async fn deliver(
        &mut self,
        _message: &oar_core::storage::postgres::AuditOutboxMessage,
    ) -> Result<AuditOutboxDelivery, ()> {
        Ok(AuditOutboxDelivery::Sent)
    }
}

impl AuthRefreshAdapter for NoopRefreshAdapter {
    fn refresh(&mut self, _snapshot: &TokenRefreshGrantSnapshot) -> RefreshOutcome {
        RefreshOutcome::TransientFailure {
            safe_error: "temporarily unavailable".to_string(),
        }
    }
}

#[async_trait::async_trait]
impl AsyncAuthRefreshAdapter for NoopRefreshAdapter {
    async fn refresh(&mut self, snapshot: &TokenRefreshGrantSnapshot) -> RefreshOutcome {
        AuthRefreshAdapter::refresh(self, snapshot)
    }
}

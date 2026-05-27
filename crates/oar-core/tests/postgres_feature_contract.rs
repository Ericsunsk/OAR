use oar_core::storage::postgres::audit_sql::{
    APPEND_AUDIT_EVENT, CLAIM_AUDIT_OUTBOX, ENQUEUE_AUDIT_OUTBOX, FIND_AUDIT_EVENTS_BY_TRACE_ID,
    MARK_AUDIT_OUTBOX_FAILED, MARK_AUDIT_OUTBOX_FAILED_FOR_ATTEMPT, MARK_AUDIT_OUTBOX_RETRYABLE,
    MARK_AUDIT_OUTBOX_RETRYABLE_FOR_ATTEMPT, MARK_AUDIT_OUTBOX_SENT,
    MARK_AUDIT_OUTBOX_SENT_FOR_ATTEMPT,
};
use oar_core::storage::postgres::device_session_sql::{
    ADVANCE_DEVICE_SESSION_CURSOR_CAS, EXPIRE_DEVICE_SESSION, GET_DEVICE_SESSION_BY_ID,
    REVOKE_DEVICE_SESSION, UPSERT_DEVICE_SESSION,
};
use oar_core::storage::postgres::identity_sql::{
    GET_LARK_IDENTITY_BY_ACTOR_EXTERNAL, GET_LARK_IDENTITY_BY_ID, GET_TENANT_BY_ID,
    GET_WORKSPACE_USER_BY_ID, UPSERT_LARK_IDENTITY, UPSERT_TENANT, UPSERT_WORKSPACE_USER,
};
use oar_core::storage::postgres::operation_ledger_sql::{
    GET_BY_IDEMPOTENCY_KEY, MARK_EXECUTING, MARK_FAILED, MARK_SUCCEEDED,
    SUBMIT_CONFIRMED_ACTION_AND_LEDGER,
};
use oar_core::storage::postgres::review_inbox_sql::{
    INSERT_EVIDENCE_ITEM, INSERT_PROPOSED_ACTION, INSERT_PROPOSED_ACTION_DECISION,
    INSERT_PROPOSED_ACTION_EVIDENCE_REF, LIST_REVIEW_INBOX_ITEMS,
    UPDATE_REVIEW_INBOX_LEDGER_PROJECTION, UPSERT_REVIEW_INBOX_ITEM,
};
use oar_core::storage::postgres::scheduler_sql::{
    CLAIM_SCHEDULER_JOB, COMPLETE_SCHEDULER_JOB_FOR_LEASE, FAIL_SCHEDULER_JOB_FOR_LEASE,
    GET_SCHEDULER_JOB, UPSERT_SCHEDULER_JOB,
};
use oar_core::storage::postgres::token_grant_sql::{
    GET_TOKEN_GRANT_BY_ID, MARK_TOKEN_GRANT_REAUTH_REQUIRED, MARK_TOKEN_GRANT_REFRESH_FAILED,
    REVOKE_TOKEN_GRANT, ROTATE_TOKEN_GRANT, UPSERT_TOKEN_GRANT,
};

fn compact(sql: &str) -> String {
    sql.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn default_build_exposes_postgres_sql_contract_constants() {
    let operation_sql = compact(SUBMIT_CONFIRMED_ACTION_AND_LEDGER);
    let transition_sql = compact(MARK_EXECUTING);
    let audit_sql = compact(APPEND_AUDIT_EVENT);
    let claim_outbox_sql = compact(CLAIM_AUDIT_OUTBOX);
    let rotate_grant_sql = compact(ROTATE_TOKEN_GRANT);
    let device_session_sql = compact(UPSERT_DEVICE_SESSION);
    let tenant_sql = compact(UPSERT_TENANT);
    let evidence_sql = compact(INSERT_EVIDENCE_ITEM);
    let review_inbox_sql = compact(UPSERT_REVIEW_INBOX_ITEM);
    let review_inbox_projection_sql = compact(UPDATE_REVIEW_INBOX_LEDGER_PROJECTION);
    let scheduler_claim_sql = compact(CLAIM_SCHEDULER_JOB);

    assert!(operation_sql.contains("insert into confirmed_actions"));
    assert!(operation_sql.contains("insert into operation_ledger"));
    assert!(operation_sql.contains("true as created"));
    assert!(operation_sql.contains("false as created"));
    assert!(transition_sql.contains("update operation_ledger"));
    assert!(audit_sql.contains("insert into audit_events"));
    assert!(claim_outbox_sql.contains("for update skip locked"));
    assert!(rotate_grant_sql.contains("update token_grants"));
    assert!(rotate_grant_sql.contains("oauth_grant_fingerprint = $3"));
    assert!(rotate_grant_sql.contains("revoked_at is null"));
    assert!(rotate_grant_sql.contains("reauth_required_at is null"));
    assert!(device_session_sql.contains("insert into device_sessions"));
    assert!(device_session_sql.contains("session_identity_hash"));
    assert!(tenant_sql.contains("insert into tenants"));
    assert!(tenant_sql.contains("on conflict (id) do update"));
    assert!(evidence_sql.contains("insert into evidence_items"));
    assert!(review_inbox_sql.contains("insert into review_inbox_items"));
    assert!(review_inbox_projection_sql.contains("update review_inbox_items"));
    assert!(scheduler_claim_sql.contains("for update skip locked"));

    // Touch all constants to lock import visibility for default builds.
    let _ = MARK_SUCCEEDED;
    let _ = MARK_FAILED;
    let _ = GET_BY_IDEMPOTENCY_KEY;
    let _ = FIND_AUDIT_EVENTS_BY_TRACE_ID;
    let _ = ENQUEUE_AUDIT_OUTBOX;
    let _ = MARK_AUDIT_OUTBOX_SENT;
    let _ = MARK_AUDIT_OUTBOX_SENT_FOR_ATTEMPT;
    let _ = MARK_AUDIT_OUTBOX_RETRYABLE;
    let _ = MARK_AUDIT_OUTBOX_RETRYABLE_FOR_ATTEMPT;
    let _ = MARK_AUDIT_OUTBOX_FAILED;
    let _ = MARK_AUDIT_OUTBOX_FAILED_FOR_ATTEMPT;
    let _ = UPSERT_TOKEN_GRANT;
    let _ = GET_TOKEN_GRANT_BY_ID;
    let _ = MARK_TOKEN_GRANT_REFRESH_FAILED;
    let _ = MARK_TOKEN_GRANT_REAUTH_REQUIRED;
    let _ = REVOKE_TOKEN_GRANT;
    let _ = ADVANCE_DEVICE_SESSION_CURSOR_CAS;
    let _ = GET_DEVICE_SESSION_BY_ID;
    let _ = REVOKE_DEVICE_SESSION;
    let _ = EXPIRE_DEVICE_SESSION;
    let _ = UPSERT_TENANT;
    let _ = GET_TENANT_BY_ID;
    let _ = UPSERT_WORKSPACE_USER;
    let _ = GET_WORKSPACE_USER_BY_ID;
    let _ = UPSERT_LARK_IDENTITY;
    let _ = GET_LARK_IDENTITY_BY_ID;
    let _ = GET_LARK_IDENTITY_BY_ACTOR_EXTERNAL;
    let _ = INSERT_PROPOSED_ACTION;
    let _ = INSERT_PROPOSED_ACTION_EVIDENCE_REF;
    let _ = INSERT_PROPOSED_ACTION_DECISION;
    let _ = LIST_REVIEW_INBOX_ITEMS;
    let _ = UPDATE_REVIEW_INBOX_LEDGER_PROJECTION;
    let _ = UPSERT_SCHEDULER_JOB;
    let _ = GET_SCHEDULER_JOB;
    let _ = COMPLETE_SCHEDULER_JOB_FOR_LEASE;
    let _ = FAIL_SCHEDULER_JOB_FOR_LEASE;
}

#[cfg(feature = "postgres")]
mod postgres_feature_api_contract {
    use super::*;
    use oar_core::action::audit_event::AuditEvent;
    use oar_core::action::confirmed_action::ConfirmedAction;
    use oar_core::action::postgres_executor::PostgresActionExecutor;
    use oar_core::action::token_refresh_audit::{
        token_refresh_audit_event, TokenRefreshAuditContext,
    };
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
        PostgresDeviceSessionRepository, PostgresExecutionRecorder,
        PostgresExecutionRecorderReport, PostgresIdentityRepository,
        PostgresLarkIdentityRepository, PostgresOperationLedgerRepository,
        PostgresReviewDecisionRecorder, PostgresReviewDecisionRecorderReport,
        PostgresReviewDecisionRecorderRequest, PostgresSchedulerJobRepository,
        PostgresTenantRepository, PostgresTokenGrantRepository, PostgresTokenRefreshOrchestrator,
        PostgresTokenRefreshRecorder, PostgresTokenRefreshScheduledSweep,
        PostgresTokenRefreshSweep, PostgresTokenRefreshSweepReport,
        PostgresTokenRefreshSweepRequest, PostgresWorkspaceUserRepository,
        RotateEncryptedGrantRequest, StoredDeviceSession, StoredLarkIdentity, StoredSchedulerJob,
        StoredTenant, StoredWorkspaceUser, TokenRefreshScheduledSweepConfig,
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
        let _retryable_for_attempt =
            PostgresAuditEventRepository::mark_outbox_retryable_for_attempt;
        let _failed = PostgresAuditEventRepository::mark_outbox_failed;
        let _failed_for_attempt = PostgresAuditEventRepository::mark_outbox_failed_for_attempt;
        let _token_refresh_mapping = token_refresh_audit_event;
        let _token_refresh_append = PostgresAuditEventRepository::append;
        let _token_refresh_audit_pipeline = (_token_refresh_mapping, _token_refresh_append);
        let _record_confirmation = PostgresExecutionRecorder::record_confirmation;
        let _record_dry_run = PostgresExecutionRecorder::record_dry_run;
        let _record_success = PostgresExecutionRecorder::record_success;
        let _record_failure = PostgresExecutionRecorder::record_failure;
        let _record_review_decision = PostgresReviewDecisionRecorder::record_decision;
        let _execute =
            PostgresActionExecutor::<MockLarkAdapter, fn() -> u64>::execute_confirmed_action;
        let _execute_with_policy =
            PostgresActionExecutor::<MockLarkAdapter, fn() -> u64>::execute_confirmed_action_with_policy;
        let _upsert_grant = PostgresTokenGrantRepository::upsert_encrypted_grant;
        let _get_grant = PostgresTokenGrantRepository::get_by_id;
        let _apply_refresh_command = PostgresTokenGrantRepository::apply_refresh_command;
        let _apply_planned_refresh_command_with_audit =
            PostgresTokenRefreshRecorder::apply_planned_command_with_audit;
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
        let _tenant_maintenance_try_ctor = PostgresTenantMaintenanceWorker::<
            NoopRefreshAdapter,
            NoopDispatcher,
            fn() -> u64,
        >::try_new;
        let _tenant_maintenance_run_once = PostgresTenantMaintenanceWorker::<
            NoopRefreshAdapter,
            NoopDispatcher,
            fn() -> u64,
        >::run_once;
        let _scheduler_upsert = PostgresSchedulerJobRepository::upsert_job;
        let _scheduler_get = PostgresSchedulerJobRepository::get_job;
        let _scheduler_try_acquire = PostgresSchedulerJobRepository::try_acquire;
        let _scheduler_complete = PostgresSchedulerJobRepository::complete_for_lease;
        let _scheduler_fail = PostgresSchedulerJobRepository::fail_for_lease;
        let _rotate_grant = PostgresTokenGrantRepository::rotate_encrypted_grant;
        let _mark_refresh_failed = PostgresTokenGrantRepository::mark_refresh_failed;
        let _mark_reauth_required = PostgresTokenGrantRepository::mark_reauth_required;
        let _revoke_grant = PostgresTokenGrantRepository::revoke;
        let _list_refresh_candidates =
            PostgresTokenGrantRepository::list_refresh_candidate_snapshots;
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
        let _phantom_review_decision_request: Option<
            PostgresReviewDecisionRecorderRequest<'static>,
        > = None;
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

    #[async_trait::async_trait(?Send)]
    impl AsyncAuthRefreshAdapter for NoopRefreshAdapter {
        async fn refresh(&mut self, snapshot: &TokenRefreshGrantSnapshot) -> RefreshOutcome {
            AuthRefreshAdapter::refresh(self, snapshot)
        }
    }

    #[test]
    fn tenant_maintenance_config_validate_rejects_fail_closed_inputs() {
        let config = PostgresTenantMaintenanceConfig {
            tenant_id: "".to_string(),
            lease_id: "lease".to_string(),
            audit_stream: "audit".to_string(),
            scheduled_lease_ms: 1,
            scheduled_retry_delay_ms: 1,
            scheduled_next_run_delay_ms: 1,
            scheduled_backlog_next_run_delay_ms: 1,
            scheduled_due_before_ms: 0,
            scheduled_limit: 1,
            scheduled_audit_trace_id: "trace".to_string(),
            scheduled_audit_sequence_start: 1,
            scheduled_actor: oar_core::action::audit_event::AuditActor {
                kind: oar_core::action::audit_event::AuditActorKind::System,
                actor_id: "maintenance".to_string(),
                display_name: None,
            },
            scheduled_workspace_id: None,
            outbox_batch_limit: 1,
            outbox_lease_ms: 1,
            outbox_retry_delay_ms: 1,
            outbox_max_attempts: 1,
        };
        assert_eq!(
            config.validate(),
            Err(PostgresTenantMaintenanceConfigValidationError::EmptyField(
                "tenant_id"
            ))
        );

        let config = PostgresTenantMaintenanceConfig {
            tenant_id: "tenant".to_string(),
            lease_id: "".to_string(),
            audit_stream: "audit".to_string(),
            scheduled_lease_ms: 1,
            scheduled_retry_delay_ms: 1,
            scheduled_next_run_delay_ms: 1,
            scheduled_backlog_next_run_delay_ms: 1,
            scheduled_due_before_ms: 0,
            scheduled_limit: 1,
            scheduled_audit_trace_id: "trace".to_string(),
            scheduled_audit_sequence_start: 1,
            scheduled_actor: oar_core::action::audit_event::AuditActor {
                kind: oar_core::action::audit_event::AuditActorKind::System,
                actor_id: "maintenance".to_string(),
                display_name: None,
            },
            scheduled_workspace_id: None,
            outbox_batch_limit: 1,
            outbox_lease_ms: 1,
            outbox_retry_delay_ms: 1,
            outbox_max_attempts: 1,
        };
        assert_eq!(
            config.validate(),
            Err(PostgresTenantMaintenanceConfigValidationError::EmptyField(
                "lease_id"
            ))
        );

        let config = PostgresTenantMaintenanceConfig {
            tenant_id: "tenant".to_string(),
            lease_id: "lease".to_string(),
            audit_stream: "".to_string(),
            scheduled_lease_ms: 1,
            scheduled_retry_delay_ms: 1,
            scheduled_next_run_delay_ms: 1,
            scheduled_backlog_next_run_delay_ms: 1,
            scheduled_due_before_ms: 0,
            scheduled_limit: 1,
            scheduled_audit_trace_id: "trace".to_string(),
            scheduled_audit_sequence_start: 1,
            scheduled_actor: oar_core::action::audit_event::AuditActor {
                kind: oar_core::action::audit_event::AuditActorKind::System,
                actor_id: "maintenance".to_string(),
                display_name: None,
            },
            scheduled_workspace_id: None,
            outbox_batch_limit: 1,
            outbox_lease_ms: 1,
            outbox_retry_delay_ms: 1,
            outbox_max_attempts: 1,
        };
        assert_eq!(
            config.validate(),
            Err(PostgresTenantMaintenanceConfigValidationError::EmptyField(
                "audit_stream"
            ))
        );

        let config = PostgresTenantMaintenanceConfig {
            tenant_id: "tenant".to_string(),
            lease_id: "lease".to_string(),
            audit_stream: "audit".to_string(),
            scheduled_lease_ms: 1,
            scheduled_retry_delay_ms: 1,
            scheduled_next_run_delay_ms: 1,
            scheduled_backlog_next_run_delay_ms: 1,
            scheduled_due_before_ms: 0,
            scheduled_limit: 1,
            scheduled_audit_trace_id: "".to_string(),
            scheduled_audit_sequence_start: 1,
            scheduled_actor: oar_core::action::audit_event::AuditActor {
                kind: oar_core::action::audit_event::AuditActorKind::System,
                actor_id: "maintenance".to_string(),
                display_name: None,
            },
            scheduled_workspace_id: None,
            outbox_batch_limit: 1,
            outbox_lease_ms: 1,
            outbox_retry_delay_ms: 1,
            outbox_max_attempts: 1,
        };
        assert_eq!(
            config.validate(),
            Err(PostgresTenantMaintenanceConfigValidationError::EmptyField(
                "scheduled_audit_trace_id"
            ))
        );

        let config = PostgresTenantMaintenanceConfig {
            tenant_id: "tenant".to_string(),
            lease_id: "lease".to_string(),
            audit_stream: "audit".to_string(),
            scheduled_lease_ms: 1,
            scheduled_retry_delay_ms: 1,
            scheduled_next_run_delay_ms: 1,
            scheduled_backlog_next_run_delay_ms: 1,
            scheduled_due_before_ms: 0,
            scheduled_limit: 0,
            scheduled_audit_trace_id: "trace".to_string(),
            scheduled_audit_sequence_start: 1,
            scheduled_actor: oar_core::action::audit_event::AuditActor {
                kind: oar_core::action::audit_event::AuditActorKind::System,
                actor_id: "maintenance".to_string(),
                display_name: None,
            },
            scheduled_workspace_id: None,
            outbox_batch_limit: 1,
            outbox_lease_ms: 1,
            outbox_retry_delay_ms: 1,
            outbox_max_attempts: 1,
        };
        assert_eq!(
            config.validate(),
            Err(
                PostgresTenantMaintenanceConfigValidationError::NonPositiveField("scheduled_limit")
            )
        );

        let config = PostgresTenantMaintenanceConfig {
            tenant_id: "tenant".to_string(),
            lease_id: "lease".to_string(),
            audit_stream: "audit".to_string(),
            scheduled_lease_ms: 1,
            scheduled_retry_delay_ms: 1,
            scheduled_next_run_delay_ms: 1,
            scheduled_backlog_next_run_delay_ms: 1,
            scheduled_due_before_ms: 0,
            scheduled_limit: 1,
            scheduled_audit_trace_id: "trace".to_string(),
            scheduled_audit_sequence_start: 1,
            scheduled_actor: oar_core::action::audit_event::AuditActor {
                kind: oar_core::action::audit_event::AuditActorKind::System,
                actor_id: "maintenance".to_string(),
                display_name: None,
            },
            scheduled_workspace_id: None,
            outbox_batch_limit: 0,
            outbox_lease_ms: 1,
            outbox_retry_delay_ms: 1,
            outbox_max_attempts: 1,
        };
        assert_eq!(
            config.validate(),
            Err(
                PostgresTenantMaintenanceConfigValidationError::NonPositiveField(
                    "outbox_batch_limit"
                )
            )
        );
    }
}

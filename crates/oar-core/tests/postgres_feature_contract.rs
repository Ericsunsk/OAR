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
    GET_LARK_IDENTITY_BY_ACTOR_EXTERNAL, GET_LARK_IDENTITY_BY_ID, GET_OAR_USER_BY_ID,
    GET_TENANT_BY_ID, UPSERT_LARK_IDENTITY, UPSERT_OAR_USER, UPSERT_TENANT,
};
use oar_core::storage::postgres::operation_ledger_sql::{
    GET_BY_IDEMPOTENCY_KEY, MARK_EXECUTING, MARK_FAILED, MARK_SUCCEEDED,
    SUBMIT_CONFIRMED_ACTION_AND_LEDGER,
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
    let _ = UPSERT_OAR_USER;
    let _ = GET_OAR_USER_BY_ID;
    let _ = UPSERT_LARK_IDENTITY;
    let _ = GET_LARK_IDENTITY_BY_ID;
    let _ = GET_LARK_IDENTITY_BY_ACTOR_EXTERNAL;
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
    use oar_core::domain::token_refresh::{
        AuthRefreshAdapter, RefreshOutcome, TokenRefreshCommandSink, TokenRefreshGrantSnapshot,
    };
    use oar_core::lark::adapter::MockLarkAdapter;
    use oar_core::storage::postgres::audit_outbox_worker::{
        AuditOutboxDelivery, AuditOutboxDispatcher, AuditOutboxDrainConfig, AuditOutboxDrainReport,
        PostgresAuditOutboxWorker,
    };
    use oar_core::storage::postgres::{
        AuditOutboxEnvelope, EncryptedTokenGrantRecord, PostgresAuditEventRepository,
        PostgresDeviceSessionRepository, PostgresExecutionUnitOfWork,
        PostgresExecutionUnitOfWorkReport, PostgresIdentityRepository,
        PostgresLarkIdentityRepository, PostgresOarUserRepository,
        PostgresOperationLedgerRepository, PostgresTenantRepository, PostgresTokenGrantRepository,
        PostgresTokenRefreshCommandSink, PostgresTokenRefreshOrchestrator,
        PostgresTokenRefreshSweep, PostgresTokenRefreshSweepReport,
        PostgresTokenRefreshSweepRequest, PostgresTokenRefreshUnitOfWork, StoredDeviceSession,
        StoredLarkIdentity, StoredOarUser, StoredTenant,
    };
    use sqlx::PgPool;

    #[test]
    fn postgres_repositories_are_importable_and_constructible_from_pg_pool() {
        let _from_pool_ctor_op: fn(PgPool) -> PostgresOperationLedgerRepository =
            PostgresOperationLedgerRepository::new;
        let _from_pool_ctor_audit: fn(PgPool) -> PostgresAuditEventRepository =
            PostgresAuditEventRepository::new;
        let _from_pool_ctor_uow: fn(PgPool) -> PostgresExecutionUnitOfWork =
            PostgresExecutionUnitOfWork::new;
        let _from_pool_ctor_token_refresh_uow: fn(PgPool) -> PostgresTokenRefreshUnitOfWork =
            PostgresTokenRefreshUnitOfWork::new;
        let _from_pool_ctor_token_grant: fn(PgPool) -> PostgresTokenGrantRepository =
            PostgresTokenGrantRepository::new;
        let _from_repository_ctor_refresh_sink: fn(
            PostgresTokenGrantRepository,
        ) -> PostgresTokenRefreshCommandSink = PostgresTokenRefreshCommandSink::new;
        let _from_pool_ctor_refresh_sink: fn(PgPool) -> PostgresTokenRefreshCommandSink =
            PostgresTokenRefreshCommandSink::from_pool;
        let _from_pool_ctor_device_session: fn(PgPool) -> PostgresDeviceSessionRepository =
            PostgresDeviceSessionRepository::new;
        let _from_pool_ctor_tenant: fn(PgPool) -> PostgresTenantRepository =
            PostgresTenantRepository::new;
        let _from_pool_ctor_oar_user: fn(PgPool) -> PostgresOarUserRepository =
            PostgresOarUserRepository::new;
        let _from_pool_ctor_lark_identity: fn(PgPool) -> PostgresLarkIdentityRepository =
            PostgresLarkIdentityRepository::new;
        let _from_pool_ctor_identity_repo: fn(PgPool) -> PostgresIdentityRepository =
            PostgresIdentityRepository::new;

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
        let _find = PostgresAuditEventRepository::find_by_trace_id;
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
        let _record_confirmation = PostgresExecutionUnitOfWork::record_confirmation;
        let _record_dry_run = PostgresExecutionUnitOfWork::record_dry_run;
        let _record_success = PostgresExecutionUnitOfWork::record_success;
        let _record_failure = PostgresExecutionUnitOfWork::record_failure;
        let _execute =
            PostgresActionExecutor::<MockLarkAdapter, fn() -> u64>::execute_confirmed_action;
        let _execute_with_policy =
            PostgresActionExecutor::<MockLarkAdapter, fn() -> u64>::execute_confirmed_action_with_policy;
        let _upsert_grant = PostgresTokenGrantRepository::upsert_encrypted_grant;
        let _get_grant = PostgresTokenGrantRepository::get_by_id;
        let _apply_refresh_command = PostgresTokenGrantRepository::apply_refresh_command;
        let _apply_refresh_command_sink =
            <PostgresTokenRefreshCommandSink as TokenRefreshCommandSink>::apply_refresh_command;
        let _apply_refresh_command_with_audit =
            PostgresTokenRefreshUnitOfWork::apply_command_with_audit;
        let _token_refresh_orchestrator_ctor =
            PostgresTokenRefreshOrchestrator::<NoopRefreshAdapter>::new;
        let _token_refresh_orchestrator_refresh =
            PostgresTokenRefreshOrchestrator::<NoopRefreshAdapter>::refresh_grant_with_audit;
        let _token_refresh_sweep_ctor = PostgresTokenRefreshSweep::<NoopRefreshAdapter>::new;
        let _token_refresh_sweep_run_once =
            PostgresTokenRefreshSweep::<NoopRefreshAdapter>::run_once_for_tenant;
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
        let _upsert_oar_user = PostgresOarUserRepository::upsert;
        let _get_oar_user = PostgresOarUserRepository::get_by_id;
        let _upsert_lark_identity = PostgresLarkIdentityRepository::upsert;
        let _get_lark_identity = PostgresLarkIdentityRepository::get_by_id;
        let _get_lark_identity_external = PostgresLarkIdentityRepository::get_by_actor_external_id;
        let _tenant_subrepo = PostgresIdentityRepository::tenants;
        let _user_subrepo = PostgresIdentityRepository::users;
        let _identity_subrepo = PostgresIdentityRepository::identities;
        assert_refresh_sink_impl::<PostgresTokenRefreshCommandSink>();

        let _phantom_action: Option<ConfirmedAction> = None;
        let _phantom_event: Option<AuditEvent> = None;
        let _phantom_envelope: Option<AuditOutboxEnvelope> = None;
        let _phantom_report: Option<PostgresExecutionUnitOfWorkReport> = None;
        let _phantom_delivery: Option<AuditOutboxDelivery> = None;
        let _phantom_drain_report: Option<AuditOutboxDrainReport> = None;
        let _phantom_config: Option<AuditOutboxDrainConfig> = None;
        let _phantom_worker: Option<PostgresAuditOutboxWorker<NoopDispatcher, fn() -> u64>> = None;
        let _phantom_session: Option<StoredDeviceSession> = None;
        let _phantom_tenant: Option<StoredTenant> = None;
        let _phantom_oar_user: Option<StoredOarUser> = None;
        let _phantom_lark_identity: Option<StoredLarkIdentity> = None;
        let _phantom_token_refresh_context: Option<TokenRefreshAuditContext> = None;
        let _phantom_token_refresh_sweep_request: Option<PostgresTokenRefreshSweepRequest> = None;
        let _phantom_token_refresh_sweep_report: Option<PostgresTokenRefreshSweepReport> = None;
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

    fn assert_refresh_sink_impl<T>()
    where
        T: TokenRefreshCommandSink<Error = oar_core::storage::postgres::PostgresRepositoryError>,
    {
    }
}

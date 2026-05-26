use oar_core::storage::postgres::audit_sql::{
    APPEND_AUDIT_EVENT, CLAIM_AUDIT_OUTBOX, ENQUEUE_AUDIT_OUTBOX, FIND_AUDIT_EVENTS_BY_TRACE_ID,
    MARK_AUDIT_OUTBOX_FAILED, MARK_AUDIT_OUTBOX_RETRYABLE, MARK_AUDIT_OUTBOX_SENT,
};
use oar_core::storage::postgres::operation_ledger_sql::{
    GET_BY_IDEMPOTENCY_KEY, MARK_EXECUTING, MARK_FAILED, MARK_SUCCEEDED,
    SUBMIT_CONFIRMED_ACTION_AND_LEDGER,
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

    assert!(operation_sql.contains("insert into confirmed_actions"));
    assert!(operation_sql.contains("insert into operation_ledger"));
    assert!(operation_sql.contains("true as created"));
    assert!(operation_sql.contains("false as created"));
    assert!(transition_sql.contains("update operation_ledger"));
    assert!(audit_sql.contains("insert into audit_events"));
    assert!(claim_outbox_sql.contains("for update skip locked"));

    // Touch all constants to lock import visibility for default builds.
    let _ = MARK_SUCCEEDED;
    let _ = MARK_FAILED;
    let _ = GET_BY_IDEMPOTENCY_KEY;
    let _ = FIND_AUDIT_EVENTS_BY_TRACE_ID;
    let _ = ENQUEUE_AUDIT_OUTBOX;
    let _ = MARK_AUDIT_OUTBOX_SENT;
    let _ = MARK_AUDIT_OUTBOX_RETRYABLE;
    let _ = MARK_AUDIT_OUTBOX_FAILED;
}

#[cfg(feature = "postgres")]
mod postgres_feature_api_contract {
    use super::*;
    use oar_core::action::audit_event::AuditEvent;
    use oar_core::action::confirmed_action::ConfirmedAction;
    use oar_core::action::postgres_executor::PostgresActionExecutor;
    use oar_core::lark::adapter::MockLarkAdapter;
    use oar_core::storage::postgres::{
        AuditOutboxEnvelope, PostgresAuditEventRepository, PostgresExecutionUnitOfWork,
        PostgresExecutionUnitOfWorkReport, PostgresOperationLedgerRepository,
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
        let _retryable = PostgresAuditEventRepository::mark_outbox_retryable;
        let _failed = PostgresAuditEventRepository::mark_outbox_failed;
        let _record_confirmation = PostgresExecutionUnitOfWork::record_confirmation;
        let _record_dry_run = PostgresExecutionUnitOfWork::record_dry_run;
        let _record_success = PostgresExecutionUnitOfWork::record_success;
        let _record_failure = PostgresExecutionUnitOfWork::record_failure;
        let _execute =
            PostgresActionExecutor::<MockLarkAdapter, fn() -> u64>::execute_confirmed_action;
        let _execute_with_policy =
            PostgresActionExecutor::<MockLarkAdapter, fn() -> u64>::execute_confirmed_action_with_policy;

        let _phantom_action: Option<ConfirmedAction> = None;
        let _phantom_event: Option<AuditEvent> = None;
        let _phantom_envelope: Option<AuditOutboxEnvelope> = None;
        let _phantom_report: Option<PostgresExecutionUnitOfWorkReport> = None;
    }
}

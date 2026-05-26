use oar_core::storage::postgres::audit_sql::{
    APPEND_AUDIT_EVENT, ENQUEUE_AUDIT_OUTBOX, FIND_AUDIT_EVENTS_BY_TRACE_ID,
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
fn submit_confirmed_action_and_ledger_uses_tenant_scoped_upsert() {
    let sql = compact(SUBMIT_CONFIRMED_ACTION_AND_LEDGER);

    assert!(sql.contains("insert into confirmed_actions"));
    assert!(sql.contains("insert into operation_ledger"));
    assert!(sql.contains("on conflict (tenant_id, idempotency_key) do nothing"));
    assert!(sql.contains("where tenant_id = $2 and idempotency_key = $4"));
    assert!(sql.contains("returning operation_id, action_id, idempotency_key, status, last_error"));
}

#[test]
fn operation_transitions_are_state_guarded() {
    let executing = compact(MARK_EXECUTING);
    let succeeded = compact(MARK_SUCCEEDED);
    let failed = compact(MARK_FAILED);

    assert!(executing.contains("update operation_ledger"));
    assert!(executing.contains("and status = 'confirmed'"));
    assert!(executing.contains("set status = 'executing'"));

    assert!(succeeded.contains("update operation_ledger"));
    assert!(succeeded.contains("and status = 'executing'"));
    assert!(succeeded.contains("set status = 'succeeded'"));

    assert!(failed.contains("update operation_ledger"));
    assert!(failed.contains("and status = 'executing'"));
    assert!(failed.contains("set status = 'failed'"));
    assert!(failed.contains("last_error = $3"));
}

#[test]
fn operation_lookup_is_tenant_scoped() {
    let sql = compact(GET_BY_IDEMPOTENCY_KEY);

    assert!(sql.contains("from operation_ledger"));
    assert!(sql.contains("where tenant_id = $1 and idempotency_key = $2"));
    assert!(sql.contains("limit 1"));
}

#[test]
fn audit_append_only_sql_is_insert_only_and_trace_ordered() {
    let append = compact(APPEND_AUDIT_EVENT);
    let query = compact(FIND_AUDIT_EVENTS_BY_TRACE_ID);

    assert!(append.starts_with("insert into audit_events"));
    assert!(!append.contains(" update "));
    assert!(!append.contains(" delete "));
    assert!(append.contains("operation_id"));
    assert!(append.contains("trace_id"));
    assert!(append.contains("sequence"));

    assert!(query.contains("from audit_events"));
    assert!(query.contains("where trace_id = $1"));
    assert!(query.contains("order by sequence asc"));
}

#[test]
fn audit_outbox_enqueue_records_pending_retry_payload() {
    let sql = compact(ENQUEUE_AUDIT_OUTBOX);

    assert!(sql.starts_with("insert into audit_outbox"));
    assert!(sql.contains("payload"));
    assert!(sql.contains("status"));
    assert!(sql.contains("'pending'"));
    assert!(sql.contains("attempt_count"));
    assert!(sql.contains("returning id"));
}

use oar_core::storage::postgres::audit_sql::{
    APPEND_AUDIT_EVENT, CLAIM_AUDIT_OUTBOX, ENQUEUE_AUDIT_OUTBOX, FIND_AUDIT_EVENTS_BY_TRACE_ID,
    MARK_AUDIT_OUTBOX_FAILED, MARK_AUDIT_OUTBOX_FAILED_FOR_ATTEMPT, MARK_AUDIT_OUTBOX_RETRYABLE,
    MARK_AUDIT_OUTBOX_RETRYABLE_FOR_ATTEMPT, MARK_AUDIT_OUTBOX_SENT,
    MARK_AUDIT_OUTBOX_SENT_FOR_ATTEMPT,
};

use crate::compact;

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
    assert!(query.contains("where tenant_id = $1"));
    assert!(query.contains("and trace_id = $2"));
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

#[test]
fn audit_outbox_claim_uses_due_pending_rows_with_skip_locked_lease() {
    let sql = compact(CLAIM_AUDIT_OUTBOX);

    assert!(sql.contains("from audit_outbox"));
    assert!(sql.contains("status = 'pending'"));
    assert!(sql.contains("tenant_id = $1"));
    assert!(sql.contains("stream = $2"));
    assert!(sql.contains("next_attempt_at is null or next_attempt_at <="));
    assert!(sql.contains("for update skip locked"));
    assert!(sql.contains("attempt_count = attempt_count + 1"));
    assert!(sql.contains("next_attempt_at = to_timestamp($5::double precision / 1000.0)"));
}

#[test]
fn audit_outbox_terminal_updates_are_tenant_scoped_and_guarded() {
    let sent = compact(MARK_AUDIT_OUTBOX_SENT);
    let retryable = compact(MARK_AUDIT_OUTBOX_RETRYABLE);
    let failed = compact(MARK_AUDIT_OUTBOX_FAILED);

    assert!(sent.contains("update audit_outbox"));
    assert!(sent.contains("where tenant_id = $1"));
    assert!(sent.contains("and id = $2"));
    assert!(sent.contains("status in ('pending', 'sent')"));
    assert!(sent.contains("set status = 'sent'"));

    assert!(retryable.contains("set status = 'pending'"));
    assert!(retryable.contains("and status = 'pending'"));
    assert!(retryable.contains("next_attempt_at = to_timestamp($3::double precision / 1000.0)"));

    assert!(failed.contains("set status = 'failed'"));
    assert!(failed.contains("and status in ('pending', 'failed')"));
    assert!(failed.contains("next_attempt_at = null"));
}

#[test]
fn audit_outbox_attempt_guarded_updates_bind_current_claim() {
    let sent = compact(MARK_AUDIT_OUTBOX_SENT_FOR_ATTEMPT);
    let retryable = compact(MARK_AUDIT_OUTBOX_RETRYABLE_FOR_ATTEMPT);
    let failed = compact(MARK_AUDIT_OUTBOX_FAILED_FOR_ATTEMPT);

    for sql in [&sent, &retryable, &failed] {
        assert!(sql.contains("where tenant_id = $1"));
        assert!(sql.contains("and id = $2"));
        assert!(sql.contains("and attempt_count = $3"));
        assert!(sql.contains("and next_attempt_at = to_timestamp($4::double precision / 1000.0)"));
        assert!(sql.contains("and status = 'pending'"));
        assert!(sql.contains("returning id"));
    }

    assert!(sent.contains("set status = 'sent'"));
    assert!(
        sent.contains("sent_at = coalesce(sent_at, to_timestamp($5::double precision / 1000.0))")
    );
    assert!(retryable.contains("set status = 'pending'"));
    assert!(retryable.contains("next_attempt_at = to_timestamp($5::double precision / 1000.0)"));
    assert!(failed.contains("set status = 'failed'"));
    assert!(failed.contains("next_attempt_at = null"));
}

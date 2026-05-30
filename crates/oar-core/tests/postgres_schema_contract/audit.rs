use super::all_sql_lowercase;

#[test]
fn audit_events_has_trace_id_and_sequence_uniqueness() {
    let sql = all_sql_lowercase();
    let has_trace_id = sql.contains("audit_events") && sql.contains("trace_id");
    let has_sequence = sql.contains("audit_events") && sql.contains("sequence");
    let has_tenant_trace_uniqueness = sql.contains("unique")
        && (sql.contains("unique (tenant_id, trace_id, sequence)")
            || sql.contains("unique(tenant_id, trace_id, sequence)"));
    let has_pair_uniqueness = sql.contains("unique")
        && sql.contains("trace_id")
        && sql.contains("sequence")
        && (sql.contains("(trace_id, sequence)")
            || sql.contains("(sequence, trace_id)")
            || sql.contains("trace_id,sequence"));

    assert!(has_trace_id, "expected audit_events.trace_id column");
    assert!(has_sequence, "expected audit_events.sequence column");
    assert!(
        has_tenant_trace_uniqueness && !has_pair_uniqueness,
        "expected tenant-scoped unique constraint on audit_events(tenant_id, trace_id, sequence)"
    );
    assert!(
        sql.contains(
            "idx_audit_events_trace_sequence on audit_events (tenant_id, trace_id, sequence)"
        ),
        "expected tenant-scoped trace lookup index"
    );
}

#[test]
fn audit_events_can_reference_operation_and_outbox_exists_for_crash_windows() {
    let sql = all_sql_lowercase();

    assert!(
        sql.contains("foreign key (tenant_id, operation_id) references operation_ledger(tenant_id, operation_id)"),
        "expected audit_events.operation_id to reference operation_ledger with tenant binding"
    );
    assert!(
        sql.contains("'proposed_action_decision_recorded'"),
        "expected audit_events to record user decisions before writeback"
    );
    assert!(
        sql.contains("create table audit_outbox"),
        "expected audit_outbox table for transaction/outbox boundary"
    );
    assert!(
        sql.contains("payload jsonb not null") && sql.contains("attempt_count"),
        "expected audit_outbox payload and retry metadata"
    );
    assert!(
        sql.contains("idx_audit_outbox_tenant_stream_pending")
            && sql.contains("on audit_outbox (tenant_id, stream, status, next_attempt_at"),
        "expected audit_outbox pending claim index to include tenant and stream"
    );
}

#[test]
fn audit_events_is_append_only_without_update_paths() {
    let sql = all_sql_lowercase();
    let has_append_only_guard = sql.contains("audit_events_no_update")
        && sql.contains("audit_events_no_delete")
        && sql.contains("prevent_audit_event_mutation");
    let has_direct_update = sql.contains("update audit_events");
    let has_direct_delete = sql.contains("delete from audit_events");

    assert!(
        has_append_only_guard,
        "expected audit_events update/delete prevention trigger"
    );
    assert!(
        !(has_direct_update || has_direct_delete),
        "expected audit_events to be append-only (no direct update/delete statement)"
    );
}

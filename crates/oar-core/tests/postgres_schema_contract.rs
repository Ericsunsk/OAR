use std::fs;
use std::path::{Path, PathBuf};

fn migration_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations")
}

fn read_all_migration_sql() -> Vec<(PathBuf, String)> {
    let dir = migration_dir();
    assert!(
        dir.exists(),
        "expected migration directory at {}",
        dir.display()
    );

    let mut sql_files = Vec::new();
    for entry in fs::read_dir(&dir).expect("failed to read migration directory") {
        let entry = entry.expect("failed to read migration directory entry");
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("sql") {
            let content = fs::read_to_string(&path)
                .unwrap_or_else(|_| panic!("failed to read {}", path.display()));
            sql_files.push((path, content));
        }
    }

    assert!(
        !sql_files.is_empty(),
        "expected at least one .sql migration file in {}",
        dir.display()
    );
    sql_files
}

fn all_sql_lowercase() -> String {
    let joined = read_all_migration_sql()
        .into_iter()
        .map(|(_, s)| s)
        .collect::<Vec<_>>()
        .join("\n");
    joined.to_lowercase()
}

#[test]
fn operation_ledger_has_unique_idempotency_key() {
    let sql = all_sql_lowercase();
    let has_inline = sql.contains("operation_ledger")
        && sql.contains("idempotency_key")
        && sql.contains("unique");
    let has_constraint = sql.contains("unique")
        && (sql.contains("(idempotency_key)") || sql.contains("idempotency_key)"));

    assert!(
        has_inline || has_constraint,
        "expected UNIQUE constraint/index for operation_ledger.idempotency_key"
    );
}

#[test]
fn confirmed_actions_and_ledger_share_idempotency_and_action_binding() {
    let sql = all_sql_lowercase();

    assert!(
        sql.contains("create table confirmed_actions"),
        "expected confirmed_actions table"
    );
    assert!(
        sql.contains("confirmed_actions")
            && sql.contains("idempotency_key")
            && sql.contains("unique (tenant_id, idempotency_key)"),
        "expected confirmed_actions tenant-scoped idempotency key uniqueness"
    );
    assert!(
        sql.contains("action_id text not null references confirmed_actions(action_id)"),
        "expected operation_ledger.action_id to reference confirmed_actions(action_id)"
    );
}

#[test]
fn audit_events_has_trace_id_and_sequence_uniqueness() {
    let sql = all_sql_lowercase();
    let has_trace_id = sql.contains("audit_events") && sql.contains("trace_id");
    let has_sequence = sql.contains("audit_events") && sql.contains("sequence");
    let has_pair_uniqueness = sql.contains("unique")
        && sql.contains("trace_id")
        && sql.contains("sequence")
        && (sql.contains("(trace_id, sequence)")
            || sql.contains("(sequence, trace_id)")
            || sql.contains("trace_id,sequence"));

    assert!(has_trace_id, "expected audit_events.trace_id column");
    assert!(has_sequence, "expected audit_events.sequence column");
    assert!(
        has_pair_uniqueness,
        "expected unique index/constraint on audit_events(trace_id, sequence)"
    );
}

#[test]
fn audit_events_can_reference_operation_and_outbox_exists_for_crash_windows() {
    let sql = all_sql_lowercase();

    assert!(
        sql.contains("operation_id text references operation_ledger(operation_id)"),
        "expected audit_events.operation_id to reference operation_ledger"
    );
    assert!(
        sql.contains("create table audit_outbox"),
        "expected audit_outbox table for transaction/outbox boundary"
    );
    assert!(
        sql.contains("payload jsonb not null") && sql.contains("attempt_count"),
        "expected audit_outbox payload and retry metadata"
    );
}

#[test]
fn token_grants_does_not_store_plaintext_access_or_refresh_tokens() {
    let sql = all_sql_lowercase();
    assert!(
        !sql.contains("access_token"),
        "found forbidden plaintext-like column/token name: access_token"
    );
    assert!(
        !sql.contains("refresh_token"),
        "found forbidden plaintext-like column/token name: refresh_token"
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

#[test]
fn device_sessions_has_sync_cursor_fields() {
    let sql = all_sql_lowercase();
    let has_table = sql.contains("device_sessions");
    let has_sync_cursor = sql.contains("sync_cursor");
    let has_cursor_updated_at = sql.contains("cursor_updated_at");

    assert!(has_table, "expected device_sessions table definition");
    assert!(
        has_sync_cursor || has_cursor_updated_at,
        "expected device_sessions sync cursor fields (e.g., sync_cursor and/or cursor_updated_at)"
    );
}

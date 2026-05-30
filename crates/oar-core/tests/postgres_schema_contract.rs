use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

fn migration_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations")
}

fn read_all_migration_sql() -> String {
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
            sql_files.push(content);
        }
    }

    assert!(
        !sql_files.is_empty(),
        "expected at least one .sql migration file in {}",
        dir.display()
    );
    sql_files.join("\n")
}

fn all_sql_lowercase() -> &'static str {
    static SQL: OnceLock<String> = OnceLock::new();
    SQL.get_or_init(|| read_all_migration_sql().to_lowercase())
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
        sql.contains("unique (tenant_id, action_id)"),
        "expected confirmed_actions to support tenant-bound downstream FKs"
    );
    assert!(
        sql.contains(
            "foreign key (tenant_id, action_id) references confirmed_actions(tenant_id, action_id)"
        ),
        "expected operation_ledger.action_id to reference confirmed_actions with tenant binding"
    );
    assert!(
        sql.contains("unique (tenant_id, operation_id)"),
        "expected operation_ledger to support tenant-bound downstream FKs"
    );
}

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

#[test]
fn review_inbox_domain_schema_stores_evidence_without_raw_content() {
    let sql = all_sql_lowercase();

    assert!(
        sql.contains("create table evidence_items"),
        "expected evidence_items table"
    );
    assert!(sql.contains("summary text not null"));
    assert!(sql.contains("source_kind text not null"));
    assert!(sql.contains("source_id text not null"));
    assert!(sql.contains("content_hash text not null"));
    assert!(sql.contains("visibility_scope text not null"));
    assert!(
        sql.contains("primary key (tenant_id, id)"),
        "expected evidence ids to be tenant-scoped for reference guards"
    );
    assert!(
        !sql.contains("raw_transcript") && !sql.contains("raw_content"),
        "evidence schema must not store raw evidence bodies"
    );
}

#[test]
fn proposed_actions_require_evidence_refs_and_single_decision() {
    let sql = all_sql_lowercase();

    assert!(
        sql.contains("create table proposed_actions"),
        "expected proposed_actions table"
    );
    assert!(
        sql.contains("primary key (tenant_id, id, version)"),
        "expected proposed_actions to support tenant-scoped versions"
    );
    assert!(
        sql.contains("create table proposed_action_evidence_refs"),
        "expected proposed_action_evidence_refs join table"
    );
    assert!(
        sql.contains("references evidence_items(tenant_id, id)"),
        "expected evidence refs to be tenant-scoped"
    );
    assert!(
        sql.contains("references proposed_actions(tenant_id, id, version) on delete cascade"),
        "expected evidence refs to cascade with proposed action versions"
    );
    assert!(
        sql.contains("create table proposed_action_decisions"),
        "expected proposed_action_decisions table"
    );
    assert!(
        sql.contains("unique (tenant_id, proposed_action_id, proposed_action_version)"),
        "expected one terminal decision per tenant/proposed action"
    );
    assert!(sql.contains("confirmed_action_id text"));
    assert!(
        sql.contains("references confirmed_actions(tenant_id, action_id)"),
        "expected decisions to reference confirmed actions with tenant binding"
    );
    assert!(
        sql.contains("(decision = 'reject' and confirmed_action_id is null)")
            && sql.contains(
                "(decision in ('confirm', 'edit_then_confirm') and confirmed_action_id is not null)"
            ),
        "expected decision confirmation guard"
    );
    assert!(
        sql.contains("(decision = 'edit_then_confirm' and edited_payload is not null)")
            && sql.contains("(decision in ('confirm', 'reject') and edited_payload is null)"),
        "expected edited payload only for edit_then_confirm"
    );
}

#[test]
fn review_inbox_items_are_cursor_projected_and_orderable() {
    let sql = all_sql_lowercase();

    assert!(
        sql.contains("create table review_inbox_items"),
        "expected review_inbox_items projection table"
    );
    assert!(sql.contains("sync_cursor_value bigint not null"));
    assert!(
        sql.contains("create sequence review_inbox_sync_cursor_seq"),
        "expected DB-owned monotonic cursor sequence for review inbox projections"
    );
    assert!(
        sql.contains("source_cursor_value bigint not null"),
        "expected source cursor to keep stale-source guards separate from DB sync cursors"
    );
    assert!(
        sql.contains("default nextval('review_inbox_sync_cursor_seq')"),
        "expected review inbox sync cursor to default to DB sequence values"
    );
    assert!(sql.contains("sort_key bigint not null"));
    assert!(sql.contains("operation_id text"));
    assert!(
        sql.contains("references workspace_users(tenant_id, id)"),
        "expected inbox/user references to be tenant-bound"
    );
    assert!(
        sql.contains("references operation_ledger(tenant_id, operation_id)"),
        "expected inbox operation projections to reference the tenant-bound ledger"
    );
    assert!(
        sql.contains("unique (tenant_id, user_id, proposed_action_id)"),
        "expected one inbox projection per user/proposed action"
    );
    assert!(
        sql.contains("unique (tenant_id, operation_id)"),
        "expected operation-bound ledger projection to update at most one inbox row"
    );
    assert!(
        sql.contains("idx_review_inbox_items_user_sort"),
        "expected sort index for weekly inbox"
    );
}

#[test]
fn scheduler_jobs_are_tenant_scoped_leased_safe_metadata() {
    let sql = all_sql_lowercase();

    assert!(
        sql.contains("create table scheduler_jobs"),
        "expected scheduler_jobs table"
    );
    assert!(sql.contains("tenant_id text not null references tenants(id)"));
    assert!(
        sql.contains("job_kind text not null check (job_kind in ('token_refresh_sweep'))"),
        "expected narrow scheduler job kind enum for Phase 0.6"
    );
    assert!(
        sql.contains("status text not null check (status in ('pending', 'running'))"),
        "expected scheduler job state guard"
    );
    assert!(sql.contains("attempt_count integer not null default 0 check (attempt_count >= 0)"));
    assert!(sql.contains("next_run_at timestamptz not null"));
    assert!(sql.contains("lease_id text"));
    assert!(sql.contains("lease_until timestamptz"));
    assert!(sql.contains("last_safe_error_code text check"));
    assert!(
        sql.contains("last_safe_error_code ~ '^[a-z0-9_:.-]{1,64}$'"),
        "expected scheduler safe error codes to be bounded machine codes"
    );
    assert!(
        sql.contains("primary key (tenant_id, job_kind)"),
        "expected one durable job row per tenant/job kind as the primary key"
    );
    assert!(
        sql.contains("unique (tenant_id, id)"),
        "expected scheduler job ids to be tenant-scoped, not globally unique"
    );
    assert!(
        sql.contains("status = 'running' and lease_id is not null and lease_until is not null")
            && sql.contains("status <> 'running' and lease_id is null and lease_until is null"),
        "expected running jobs to hold lease metadata and non-running jobs to clear it"
    );
    assert!(
        sql.contains("idx_scheduler_jobs_due"),
        "expected scheduler claim index"
    );
    assert!(
        !sql.contains("scheduler_jobs") || !sql.contains("raw_stdout"),
        "scheduler metadata must not store raw adapter stdout"
    );
    assert!(
        !sql.contains("raw_stderr"),
        "scheduler metadata must not store raw adapter stderr"
    );
}

#[test]
fn identity_and_action_domain_foreign_keys_are_tenant_bound() {
    let sql = all_sql_lowercase();

    assert!(
        sql.contains("unique (tenant_id, id)") && sql.contains("create table lark_identities"),
        "expected lark_identities to expose tenant-bound key for downstream foreign keys"
    );
    assert!(
        sql.contains(
            "foreign key (tenant_id, identity_id) references lark_identities(tenant_id, id)"
        ),
        "expected token_grants.identity_id to be tenant-bound"
    );
    assert!(
        sql.contains("foreign key (tenant_id, user_id) references workspace_users(tenant_id, id)"),
        "expected device_sessions.user_id to be tenant-bound"
    );
    assert!(
        sql.contains(
            "foreign key (tenant_id, actor_user_id) references workspace_users(tenant_id, id)"
        ),
        "expected confirmed_actions.actor_user_id to be tenant-bound"
    );
}

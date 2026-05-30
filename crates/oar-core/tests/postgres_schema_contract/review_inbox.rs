use super::all_sql_lowercase;

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

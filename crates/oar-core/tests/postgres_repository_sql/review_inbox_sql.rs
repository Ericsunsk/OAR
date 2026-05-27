use oar_core::storage::postgres::review_inbox_sql::{
    INSERT_EVIDENCE_ITEM, INSERT_PROPOSED_ACTION, INSERT_PROPOSED_ACTION_DECISION,
    INSERT_PROPOSED_ACTION_EVIDENCE_REF, LIST_REVIEW_INBOX_ITEMS,
    UPDATE_REVIEW_INBOX_LEDGER_PROJECTION, UPSERT_REVIEW_INBOX_ITEM,
};

use crate::compact;

#[test]
fn evidence_insert_stores_summary_reference_hash_and_no_raw_content() {
    let sql = compact(INSERT_EVIDENCE_ITEM);

    assert!(sql.contains("insert into evidence_items"));
    assert!(sql.contains("summary"));
    assert!(sql.contains("source_kind"));
    assert!(sql.contains("source_id"));
    assert!(sql.contains("content_hash"));
    assert!(sql.contains("visibility_scope"));
    assert!(!sql.contains("raw_transcript"));
    assert!(!sql.contains("raw_content"));
    assert!(!sql.contains("access_token"));
    assert!(!sql.contains("refresh_token"));
}

#[test]
fn proposed_action_insert_keeps_payload_and_evidence_refs_separate() {
    let action_sql = compact(INSERT_PROPOSED_ACTION);
    let evidence_ref_sql = compact(INSERT_PROPOSED_ACTION_EVIDENCE_REF);

    assert!(action_sql.contains("insert into proposed_actions"));
    assert!(action_sql.contains("suggested_payload"));
    assert!(action_sql.contains("on conflict ("));
    assert!(action_sql.contains("tenant_id"));
    assert!(action_sql.contains("id"));
    assert!(action_sql.contains("version"));
    assert!(action_sql.contains("do nothing"));

    assert!(evidence_ref_sql.contains("insert into proposed_action_evidence_refs"));
    assert!(evidence_ref_sql.contains("proposed_action_id"));
    assert!(evidence_ref_sql.contains("evidence_id"));
    assert!(evidence_ref_sql.contains("proposed_action_version"));
    assert!(evidence_ref_sql
        .contains("on conflict (tenant_id, proposed_action_id, proposed_action_version, evidence_id) do nothing"));
}

#[test]
fn proposed_action_decision_is_tenant_scoped_and_single_terminal() {
    let sql = compact(INSERT_PROPOSED_ACTION_DECISION);

    assert!(sql.contains("insert into proposed_action_decisions"));
    assert!(sql.contains("tenant_id"));
    assert!(sql.contains("proposed_action_id"));
    assert!(sql.contains("proposed_action_version"));
    assert!(sql.contains("confirmed_action_id"));
    assert!(sql.contains(
        "on conflict (tenant_id, proposed_action_id, proposed_action_version) do nothing"
    ));
}

#[test]
fn review_inbox_upsert_is_cursor_guarded_and_terminal_guarded() {
    let sql = compact(UPSERT_REVIEW_INBOX_ITEM);

    assert!(sql.contains("insert into review_inbox_items"));
    assert!(sql.contains("ledger_status"));
    assert!(sql.contains("operation_id"));
    assert!(sql.contains("on conflict (tenant_id, user_id, proposed_action_id) do update"));
    assert!(sql.contains("ledger_status = excluded.ledger_status"));
    assert!(sql.contains("operation_id = excluded.operation_id"));
    assert!(sql.contains("source_cursor_value = excluded.source_cursor_value"));
    assert!(sql.contains("nextval('review_inbox_sync_cursor_seq')"));
    assert!(sql.contains("review_inbox_items.source_cursor_value < $10"));
    assert!(sql.contains(
        "review_inbox_items.status not in ('rejected', 'succeeded', 'failed', 'withdrawn')"
    ));
}

#[test]
fn review_inbox_ledger_projection_is_operation_scoped_and_guarded() {
    let sql = compact(UPDATE_REVIEW_INBOX_LEDGER_PROJECTION);

    assert!(sql.starts_with("update review_inbox_items"));
    assert!(sql.contains("set status = $3"));
    assert!(sql.contains("ledger_status = $4"));
    assert!(!sql.contains("source_cursor_value ="));
    assert!(sql.contains("updated_at = to_timestamp($5::double precision / 1000.0)"));
    assert!(sql.contains("nextval('review_inbox_sync_cursor_seq')"));
    assert!(sql.contains("sync_cursor_value + 1"));
    assert!(sql.contains("where tenant_id = $1"));
    assert!(sql.contains("and operation_id = $2"));
    assert!(sql.contains("status not in ('rejected', 'succeeded', 'failed', 'withdrawn')"));
    assert!(sql.contains(
        "coalesce(ledger_status, 'confirmed') not in ('succeeded', 'failed', 'cancelled')"
    ));
    assert!(sql.contains("returning id"));
}

#[test]
fn review_inbox_list_is_incremental_and_ordered_for_weekly_inbox() {
    let sql = compact(LIST_REVIEW_INBOX_ITEMS);

    assert!(sql.contains("from review_inbox_items"));
    assert!(sql.contains("where tenant_id = $1"));
    assert!(sql.contains("and user_id = $2"));
    assert!(sql.contains("and sync_cursor_value > $3"));
    assert!(sql.contains("order by sort_key desc, updated_at desc, id asc"));
    assert!(sql.contains("limit $4"));
}

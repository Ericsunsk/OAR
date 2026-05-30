use oar_core::storage::postgres::operation_ledger_sql::{
    GET_BY_IDEMPOTENCY_KEY, LIST_CONFIRMED_ACTIONS_READY_FOR_EXECUTION, MARK_EXECUTING,
    MARK_FAILED, MARK_SUCCEEDED, SUBMIT_CONFIRMED_ACTION_AND_LEDGER,
};

use crate::compact;

#[test]
fn submit_confirmed_action_and_ledger_uses_tenant_scoped_upsert() {
    let sql = compact(SUBMIT_CONFIRMED_ACTION_AND_LEDGER);

    assert!(sql.contains("insert into confirmed_actions"));
    assert!(sql.contains("insert into operation_ledger"));
    assert!(sql.contains("tenant_id"));
    assert!(sql.contains("on conflict (tenant_id, idempotency_key) do nothing"));
    assert!(sql.contains("where tenant_id = $2 and idempotency_key = $4"));
    assert!(
        sql.contains("true as created") && sql.contains("false as created"),
        "submit SQL should expose an explicit created flag instead of inferring from operation_id"
    );
}

#[test]
fn operation_transitions_are_state_guarded() {
    let executing = compact(MARK_EXECUTING);
    let succeeded = compact(MARK_SUCCEEDED);
    let failed = compact(MARK_FAILED);

    assert!(executing.contains("update operation_ledger"));
    assert!(executing.contains("returning operation_id, tenant_id"));
    assert!(executing.contains("and status = 'confirmed'"));
    assert!(executing.contains("set status = 'executing'"));

    assert!(succeeded.contains("update operation_ledger"));
    assert!(succeeded.contains("returning operation_id, tenant_id"));
    assert!(succeeded.contains("and status = 'executing'"));
    assert!(succeeded.contains("set status = 'succeeded'"));

    assert!(failed.contains("update operation_ledger"));
    assert!(failed.contains("returning operation_id, tenant_id"));
    assert!(failed.contains("and status = 'executing'"));
    assert!(failed.contains("set status = 'failed'"));
    assert!(failed.contains("last_error = $3"));
}

#[test]
fn operation_lookup_is_tenant_scoped() {
    let sql = compact(GET_BY_IDEMPOTENCY_KEY);

    assert!(sql.contains("from operation_ledger"));
    assert!(sql.contains("select operation_id, tenant_id"));
    assert!(sql.contains("where tenant_id = $1 and idempotency_key = $2"));
    assert!(sql.contains("limit 1"));
}

#[test]
fn confirmed_action_execution_queue_is_tenant_scoped_and_ordered() {
    let sql = compact(LIST_CONFIRMED_ACTIONS_READY_FOR_EXECUTION);

    assert!(sql.contains("from operation_ledger"));
    assert!(sql.contains("join confirmed_actions"));
    assert!(sql.contains("join proposed_action_decisions"));
    assert!(sql.contains("join proposed_actions"));
    assert!(sql.contains("left join lateral"));
    assert!(sql.contains("from proposed_action_evidence_refs"));
    assert!(sql.contains("confirmed_actions.idempotency_key = operation_ledger.idempotency_key"));
    assert!(
        sql.contains("proposed_action_decisions.confirmed_action_id = confirmed_actions.action_id")
    );
    assert!(sql.contains("proposed_actions.id = proposed_action_decisions.proposed_action_id"));
    assert!(sql.contains("operation_ledger.tenant_id = $1"));
    assert!(sql.contains("operation_ledger.status = 'confirmed'"));
    assert!(sql.contains("confirmed_actions.status = 'confirmed'"));
    assert!(sql.contains("proposed_action_decisions.decision in ('confirm', 'edit_then_confirm')"));
    assert!(sql.contains("array_agg(proposed_action_evidence_refs.evidence_id"));
    assert!(sql.contains("order by operation_ledger.created_at asc"));
    assert!(sql.contains("limit $2"));
}

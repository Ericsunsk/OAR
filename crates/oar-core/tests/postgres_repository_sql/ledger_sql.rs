use oar_core::storage::postgres::operation_ledger_sql::{
    GET_BY_IDEMPOTENCY_KEY, MARK_EXECUTING, MARK_FAILED, MARK_SUCCEEDED,
    SUBMIT_CONFIRMED_ACTION_AND_LEDGER,
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

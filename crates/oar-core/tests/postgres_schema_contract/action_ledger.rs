use super::all_sql_lowercase;

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

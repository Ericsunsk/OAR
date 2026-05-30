use super::all_sql_lowercase;

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

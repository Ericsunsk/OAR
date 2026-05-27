use oar_core::storage::postgres::identity_sql::{
    GET_LARK_IDENTITY_BY_ACTOR_EXTERNAL, GET_LARK_IDENTITY_BY_ID, GET_TENANT_BY_ID,
    GET_WORKSPACE_USER_BY_ID, UPSERT_LARK_IDENTITY, UPSERT_TENANT, UPSERT_WORKSPACE_USER,
};

use crate::compact;

#[test]
fn identity_upsert_sql_uses_id_conflict_path_with_tenant_guard() {
    let upsert = compact(UPSERT_LARK_IDENTITY);
    let get_by_external = compact(GET_LARK_IDENTITY_BY_ACTOR_EXTERNAL);

    assert!(upsert.contains("insert into lark_identities"));
    assert!(upsert.contains("on conflict (id) do update"));
    assert!(upsert.contains("where lark_identities.tenant_id = excluded.tenant_id"));
    assert!(upsert.contains("actor_external_id = excluded.actor_external_id"));
    assert!(upsert.contains("and not exists (select 1 from upserted)"));

    assert!(get_by_external.contains("from lark_identities"));
    assert!(get_by_external.contains("where tenant_id = $1"));
    assert!(get_by_external.contains("and actor_kind = $2"));
    assert!(get_by_external.contains("and actor_external_id = $3"));
    assert!(get_by_external.contains("limit 1"));
}

#[test]
fn identity_sql_is_tenant_scoped_and_conflict_guarded() {
    let upsert_tenant = compact(UPSERT_TENANT);
    let get_tenant = compact(GET_TENANT_BY_ID);
    let upsert_user = compact(UPSERT_WORKSPACE_USER);
    let get_user = compact(GET_WORKSPACE_USER_BY_ID);
    let upsert_identity = compact(UPSERT_LARK_IDENTITY);
    let get_identity = compact(GET_LARK_IDENTITY_BY_ID);
    let get_identity_external = compact(GET_LARK_IDENTITY_BY_ACTOR_EXTERNAL);

    assert!(upsert_tenant.contains("insert into tenants"));
    assert!(upsert_tenant.contains("on conflict (id) do update"));
    assert!(upsert_tenant.contains("status"));
    assert!(get_tenant.contains("from tenants"));
    assert!(get_tenant.contains("where id = $1"));
    assert!(get_tenant.contains("limit 1"));

    assert!(upsert_user.contains("insert into workspace_users"));
    assert!(upsert_user.contains("on conflict (id) do update"));
    assert!(upsert_user.contains("where workspace_users.tenant_id = excluded.tenant_id"));
    assert!(upsert_user.contains("not exists (select 1 from upserted)"));
    assert!(get_user.contains("from workspace_users"));
    assert!(get_user.contains("where tenant_id = $1"));
    assert!(get_user.contains("and id = $2"));
    assert!(get_user.contains("limit 1"));

    assert!(upsert_identity.contains("insert into lark_identities"));
    assert!(upsert_identity.contains("on conflict (id) do update"));
    assert!(upsert_identity.contains("where lark_identities.tenant_id = excluded.tenant_id"));
    assert!(upsert_identity.contains("not exists (select 1 from upserted)"));
    assert!(get_identity.contains("from lark_identities"));
    assert!(get_identity.contains("where tenant_id = $1"));
    assert!(get_identity.contains("and id = $2"));
    assert!(get_identity.contains("limit 1"));

    assert!(get_identity_external.contains("from lark_identities"));
    assert!(get_identity_external.contains("where tenant_id = $1"));
    assert!(get_identity_external.contains("and actor_kind = $2"));
    assert!(get_identity_external.contains("and actor_external_id = $3"));
    assert!(get_identity_external.contains("limit 1"));
}

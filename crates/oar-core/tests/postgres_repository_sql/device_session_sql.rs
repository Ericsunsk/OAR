use oar_core::storage::postgres::device_session_sql::{
    ADVANCE_DEVICE_SESSION_CURSOR_CAS, EXPIRE_DEVICE_SESSION, GET_DEVICE_SESSION_BY_ID,
    REVOKE_DEVICE_SESSION, UPSERT_DEVICE_SESSION,
};

use crate::compact;

#[test]
fn device_session_sql_is_tenant_scoped_and_state_guarded() {
    let upsert = compact(UPSERT_DEVICE_SESSION);
    let get = compact(GET_DEVICE_SESSION_BY_ID);
    let advance = compact(ADVANCE_DEVICE_SESSION_CURSOR_CAS);
    let revoke = compact(REVOKE_DEVICE_SESSION);
    let expire = compact(EXPIRE_DEVICE_SESSION);

    assert!(upsert.contains("insert into device_sessions"));
    assert!(upsert.contains("session_identity_hash"));
    assert!(upsert.contains("on conflict (id) do update"));
    assert!(upsert.contains("where device_sessions.tenant_id = excluded.tenant_id"));
    assert!(upsert.contains("and device_sessions.state = 'active'"));
    assert!(upsert.contains("and device_sessions.revoked_at is null"));
    assert!(upsert.contains("and device_sessions.expired_at is null"));
    assert!(!upsert.contains("state = excluded.state"));
    assert!(!upsert.contains("revoked_at = excluded.revoked_at"));
    assert!(!upsert.contains("expired_at = excluded.expired_at"));
    assert!(upsert.contains("and not exists (select 1 from upserted)"));

    assert!(get.contains("from device_sessions"));
    assert!(get.contains("where tenant_id = $1"));
    assert!(get.contains("and id = $2"));
    assert!(get.contains("limit 1"));

    assert!(advance.contains("update device_sessions"));
    assert!(advance.contains("where tenant_id = $1"));
    assert!(advance.contains("and id = $2"));
    assert!(advance.contains("and sync_cursor_value = $5"));
    assert!(advance.contains("and $3 > sync_cursor_value"));
    assert!(advance.contains("and state = 'active'"));
    assert!(advance.contains("and revoked_at is null"));
    assert!(advance.contains("and expired_at is null"));

    assert!(revoke.contains("update device_sessions"));
    assert!(revoke.contains("set state = 'revoked'"));
    assert!(revoke.contains("where tenant_id = $1"));
    assert!(revoke.contains("and id = $2"));
    assert!(revoke.contains("and state <> 'revoked'"));

    assert!(expire.contains("update device_sessions"));
    assert!(expire.contains("set state = 'expired'"));
    assert!(expire.contains("where tenant_id = $1"));
    assert!(expire.contains("and id = $2"));
    assert!(expire.contains("and state = 'active'"));
    assert!(expire.contains("and revoked_at is null"));
}

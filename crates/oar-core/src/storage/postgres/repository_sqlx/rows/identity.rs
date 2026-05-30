use super::*;

pub(in crate::storage::postgres::repository_sqlx) fn stored_tenant_from_row(
    row: &PgRow,
) -> PgRepositoryResult<StoredTenant> {
    let status: String = row.try_get("status")?;
    Ok(StoredTenant {
        id: row.try_get("id")?,
        display_name: row.try_get("display_name")?,
        status: tenant_status_from_db(&status)?,
    })
}

pub(in crate::storage::postgres::repository_sqlx) fn stored_workspace_user_from_row(
    row: &PgRow,
) -> PgRepositoryResult<StoredWorkspaceUser> {
    let status: String = row.try_get("status")?;
    Ok(StoredWorkspaceUser {
        id: row.try_get("id")?,
        tenant_id: row.try_get("tenant_id")?,
        display_name: row.try_get("display_name")?,
        status: workspace_user_status_from_db(&status)?,
    })
}

pub(in crate::storage::postgres::repository_sqlx) fn stored_lark_identity_from_row(
    row: &PgRow,
) -> PgRepositoryResult<StoredLarkIdentity> {
    let actor_kind: String = row.try_get("actor_kind")?;
    Ok(StoredLarkIdentity {
        id: row.try_get("id")?,
        tenant_id: row.try_get("tenant_id")?,
        actor_kind: identity_actor_kind_from_db(&actor_kind)?,
        actor_external_id: row.try_get("actor_external_id")?,
        display_name: row.try_get("display_name")?,
    })
}

pub(in crate::storage::postgres::repository_sqlx) fn stored_device_session_from_row(
    row: &PgRow,
) -> PgRepositoryResult<StoredDeviceSession> {
    let entry_point: String = row.try_get("entry_point")?;
    let state: String = row.try_get("state")?;

    Ok(StoredDeviceSession {
        id: row.try_get("id")?,
        tenant_id: row.try_get("tenant_id")?,
        user_id: row.try_get("user_id")?,
        entry_point: device_entry_point_from_db(&entry_point)?,
        state: device_session_state_from_db(&state)?,
        sync_stream: row.try_get("sync_stream")?,
        sync_cursor_value: non_negative_i64_to_u64(
            row.try_get("sync_cursor_value")?,
            "sync_cursor_value",
        )?,
        sync_cursor_updated_at: ms_to_system_time(non_negative_i64_to_u64(
            row.try_get("sync_cursor_updated_at_ms")?,
            "sync_cursor_updated_at_ms",
        )?),
        session_identity_hash: row.try_get("session_identity_hash")?,
        last_seen_at: ms_to_system_time(non_negative_i64_to_u64(
            row.try_get("last_seen_at_ms")?,
            "last_seen_at_ms",
        )?),
        revoked_at: optional_non_negative_i64_to_u64(
            row.try_get("revoked_at_ms")?,
            "revoked_at_ms",
        )?
        .map(ms_to_system_time),
        expired_at: optional_non_negative_i64_to_u64(
            row.try_get("expired_at_ms")?,
            "expired_at_ms",
        )?
        .map(ms_to_system_time),
    })
}

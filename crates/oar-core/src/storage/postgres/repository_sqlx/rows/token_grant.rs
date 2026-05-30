use super::*;

pub(in crate::storage::postgres::repository_sqlx) fn encrypted_token_grant_from_row(
    row: &PgRow,
) -> PgRepositoryResult<EncryptedTokenGrantRecord> {
    let actor_kind: String = row.try_get("actor_kind")?;
    let scope_boundary: String = row.try_get("scope_boundary")?;
    let state: String = row.try_get("state")?;

    Ok(EncryptedTokenGrantRecord {
        id: row.try_get("id")?,
        tenant_id: row.try_get("tenant_id")?,
        identity_id: row.try_get("identity_id")?,
        actor_kind: identity_actor_kind_from_db(&actor_kind)?,
        scope_boundary: scope_boundary_from_db(&scope_boundary)?,
        scopes: row.try_get("scopes")?,
        state: token_grant_state_from_db(&state)?,
        issued_at_ms: non_negative_i64_to_u64(row.try_get("issued_at_ms")?, "issued_at_ms")?,
        expires_at_ms: optional_non_negative_i64_to_u64(
            row.try_get("expires_at_ms")?,
            "expires_at_ms",
        )?,
        refreshed_at_ms: optional_non_negative_i64_to_u64(
            row.try_get("refreshed_at_ms")?,
            "refreshed_at_ms",
        )?,
        revoked_at_ms: optional_non_negative_i64_to_u64(
            row.try_get("revoked_at_ms")?,
            "revoked_at_ms",
        )?,
        reauth_required_at_ms: optional_non_negative_i64_to_u64(
            row.try_get("reauth_required_at_ms")?,
            "reauth_required_at_ms",
        )?,
        last_refresh_error: row.try_get("last_refresh_error")?,
        encrypted_oauth_grant: row.try_get("encrypted_oauth_grant")?,
        oauth_grant_key_id: row.try_get("oauth_grant_key_id")?,
        oauth_grant_fingerprint: row.try_get("oauth_grant_fingerprint")?,
        revocation_reason: row.try_get("revocation_reason")?,
    })
}

pub(in crate::storage::postgres::repository_sqlx) fn token_refresh_snapshot_from_row(
    row: &PgRow,
) -> PgRepositoryResult<TokenRefreshGrantSnapshot> {
    let state: String = row.try_get("state")?;
    Ok(TokenRefreshGrantSnapshot {
        grant_id: TokenGrantId(row.try_get("id")?),
        tenant_id: TenantId(row.try_get("tenant_id")?),
        expected_fingerprint: row.try_get("oauth_grant_fingerprint")?,
        state: token_grant_state_from_db(&state)?,
        has_refresh_material: row.try_get("has_refresh_material")?,
        revoked_at: optional_non_negative_i64_to_u64(
            row.try_get("revoked_at_ms")?,
            "revoked_at_ms",
        )?
        .map(ms_to_system_time),
        reauth_required_at: optional_non_negative_i64_to_u64(
            row.try_get("reauth_required_at_ms")?,
            "reauth_required_at_ms",
        )?
        .map(ms_to_system_time),
    })
}

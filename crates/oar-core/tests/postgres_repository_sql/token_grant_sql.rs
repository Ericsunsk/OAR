use oar_core::storage::postgres::token_grant_sql::{
    GET_TOKEN_GRANT_BY_ID, LIST_TOKEN_REFRESH_CANDIDATE_SNAPSHOTS,
    LOCK_PAUSED_TOKEN_GRANT_REFRESH_FOR_RECOVERY, MARK_TOKEN_GRANT_REAUTH_REQUIRED,
    MARK_TOKEN_GRANT_REFRESH_FAILED, RESUME_PAUSED_TOKEN_GRANT_REFRESH_FOR_RECOVERY,
    REVOKE_TOKEN_GRANT, ROTATE_TOKEN_GRANT, UPSERT_TOKEN_GRANT,
};

use crate::compact;

fn assert_token_grant_cas_guard(sql: &str) {
    assert!(sql.contains("where tenant_id = $1"));
    assert!(sql.contains("and id = $2"));
    assert!(sql.contains("and oauth_grant_fingerprint = $3"));
    assert!(sql.contains("and state in ('valid', 'needs_refresh', 'expired')"));
    assert!(sql.contains("and revoked_at is null"));
    assert!(sql.contains("and reauth_required_at is null"));
    assert!(sql.contains("returning"));
}

#[test]
fn token_grant_sql_uses_encrypted_grant_material_only() {
    for sql in [
        UPSERT_TOKEN_GRANT,
        GET_TOKEN_GRANT_BY_ID,
        ROTATE_TOKEN_GRANT,
        MARK_TOKEN_GRANT_REFRESH_FAILED,
        MARK_TOKEN_GRANT_REAUTH_REQUIRED,
        REVOKE_TOKEN_GRANT,
    ] {
        let compacted = compact(sql);

        assert!(compacted.contains("encrypted_oauth_grant"));
        assert!(compacted.contains("oauth_grant_key_id"));
        assert!(compacted.contains("oauth_grant_fingerprint"));
        assert!(
            !compacted.contains("access_token"),
            "TokenGrant SQL must not expose plaintext access token fields"
        );
        assert!(
            !compacted.contains("refresh_token"),
            "TokenGrant SQL must not expose plaintext refresh token fields"
        );
    }
}

#[test]
fn token_grant_rotation_is_cas_guarded_and_clears_refresh_error() {
    let sql = compact(ROTATE_TOKEN_GRANT);

    assert!(sql.contains("update token_grants"));
    assert!(sql.contains("set state = 'valid'"));
    assert!(sql.contains("expires_at = case when $4::bigint is null"));
    assert!(sql.contains("refreshed_at = to_timestamp($5::double precision / 1000.0)"));
    assert!(sql.contains("last_refresh_error = null"));
    assert!(sql.contains("encrypted_oauth_grant = $6"));
    assert!(sql.contains("oauth_grant_key_id = $7"));
    assert!(sql.contains("oauth_grant_fingerprint = $8"));
    assert_token_grant_cas_guard(&sql);
}

#[test]
fn token_grant_refresh_failure_and_reauth_marks_are_guarded() {
    let refresh_failed = compact(MARK_TOKEN_GRANT_REFRESH_FAILED);
    let reauth_required = compact(MARK_TOKEN_GRANT_REAUTH_REQUIRED);

    for sql in [&refresh_failed, &reauth_required] {
        assert!(sql.contains("update token_grants"));
        assert!(sql.contains("last_refresh_error = $5"));
        assert_token_grant_cas_guard(sql);
    }

    assert!(refresh_failed.contains("set state = 'needs_refresh'"));
    assert!(refresh_failed.contains("refreshed_at = to_timestamp($4::double precision / 1000.0)"));
    assert!(reauth_required.contains("set state = 'reauth_required'"));
    assert!(reauth_required
        .contains("reauth_required_at = to_timestamp($4::double precision / 1000.0)"));
}

#[test]
fn token_grant_lookup_revoke_and_upsert_are_tenant_scoped() {
    let get = compact(GET_TOKEN_GRANT_BY_ID);
    let revoke = compact(REVOKE_TOKEN_GRANT);
    let upsert = compact(UPSERT_TOKEN_GRANT);

    assert!(get.contains("from token_grants"));
    assert!(get.contains("where tenant_id = $1 and id = $2"));
    assert!(get.contains("limit 1"));

    assert!(revoke.contains("update token_grants"));
    assert!(revoke.contains("set state = 'revoked'"));
    assert!(revoke.contains("where tenant_id = $1"));
    assert!(revoke.contains("and id = $2"));
    assert!(revoke.contains("and state <> 'revoked'"));

    assert!(upsert.contains("insert into token_grants"));
    assert!(upsert.contains("on conflict (id) do update"));
    assert!(upsert.contains("where token_grants.tenant_id = excluded.tenant_id"));
}

#[test]
fn token_refresh_candidate_sql_contract_is_tenant_scoped_guarded_and_deterministic() {
    let sql = compact(LIST_TOKEN_REFRESH_CANDIDATE_SNAPSHOTS);

    assert!(sql.contains("from token_grants"));
    assert!(sql.contains("where tenant_id = $1"));
    assert!(sql.contains("state in ('valid', 'needs_refresh', 'expired')"));
    assert!(sql.contains("and revoked_at is null"));
    assert!(sql.contains("and reauth_required_at is null"));
    assert!(sql.contains("coalesce(last_refresh_error, '') not in"));
    assert!(sql.contains("'refresh_config_required'"));
    assert!(sql.contains("'auth_refresh_parse_failed'"));
    assert!(sql.contains("'auth_refresh_oversized_response'"));
    assert!(sql.contains("octet_length(encrypted_oauth_grant) > 0"));
    assert!(sql.contains("state in ('needs_refresh', 'expired') or expires_at <= to_timestamp($2::double precision / 1000.0)"));
    assert!(sql.contains("order by"));
    assert!(sql.contains("case when state in ('needs_refresh', 'expired') then 0 else 1 end"));
    assert!(sql.contains("expires_at asc nulls first"));
    assert!(sql.contains("id asc"));
    assert!(sql.contains("limit $3"));
    assert!(!sql.contains("encrypted_oauth_grant,"));
}

#[test]
fn token_grant_paused_refresh_recovery_is_single_row_state_guarded_and_secret_safe() {
    let lock = compact(LOCK_PAUSED_TOKEN_GRANT_REFRESH_FOR_RECOVERY);
    let resume = compact(RESUME_PAUSED_TOKEN_GRANT_REFRESH_FOR_RECOVERY);

    assert!(lock.starts_with("select "));
    assert!(lock.contains("where tenant_id = $1"));
    assert!(lock.contains("and id = $2"));
    assert!(lock.contains("floor(extract(epoch from updated_at) * 1000)::bigint = $3"));
    assert!(lock.contains("state in ('valid', 'needs_refresh', 'expired')"));
    assert!(lock.contains("and revoked_at is null"));
    assert!(lock.contains("and reauth_required_at is null"));
    assert!(lock.contains("coalesce(last_refresh_error, '') in"));
    assert!(lock.contains("'refresh_config_required'"));
    assert!(lock.contains("'auth_refresh_parse_failed'"));
    assert!(lock.contains("'auth_refresh_oversized_response'"));
    assert!(lock.contains("and octet_length(encrypted_oauth_grant) > 0"));
    assert!(lock.contains("for update"));

    assert!(resume.starts_with("update token_grants"));
    assert!(resume.contains("last_refresh_error = null"));
    assert!(resume.contains("updated_at = to_timestamp($4::double precision / 1000.0)"));
    assert!(resume.contains("where tenant_id = $1"));
    assert!(resume.contains("and id = $2"));
    assert!(resume.contains("floor(extract(epoch from updated_at) * 1000)::bigint = $3"));
    assert!(resume.contains("state in ('valid', 'needs_refresh', 'expired')"));
    assert!(resume.contains("and revoked_at is null"));
    assert!(resume.contains("and reauth_required_at is null"));
    assert!(resume.contains("coalesce(last_refresh_error, '') in"));
    assert!(resume.contains("returning id"));

    for sql in [lock, resume] {
        assert!(!sql.contains("access_token"));
        assert!(!sql.contains("refresh_token"));
        assert!(!sql.contains("oauth_grant_key_id"));
        assert!(!sql.contains("oauth_grant_fingerprint"));
        assert!(!sql.contains("encrypted_oauth_grant,"));
    }
}

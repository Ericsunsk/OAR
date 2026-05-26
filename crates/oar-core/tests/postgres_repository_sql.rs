use oar_core::storage::postgres::audit_sql::{
    APPEND_AUDIT_EVENT, CLAIM_AUDIT_OUTBOX, ENQUEUE_AUDIT_OUTBOX, FIND_AUDIT_EVENTS_BY_TRACE_ID,
    MARK_AUDIT_OUTBOX_FAILED, MARK_AUDIT_OUTBOX_FAILED_FOR_ATTEMPT, MARK_AUDIT_OUTBOX_RETRYABLE,
    MARK_AUDIT_OUTBOX_RETRYABLE_FOR_ATTEMPT, MARK_AUDIT_OUTBOX_SENT,
    MARK_AUDIT_OUTBOX_SENT_FOR_ATTEMPT,
};
use oar_core::storage::postgres::device_session_sql::{
    ADVANCE_DEVICE_SESSION_CURSOR_CAS, EXPIRE_DEVICE_SESSION, GET_DEVICE_SESSION_BY_ID,
    REVOKE_DEVICE_SESSION, UPSERT_DEVICE_SESSION,
};
use oar_core::storage::postgres::identity_sql::{
    GET_LARK_IDENTITY_BY_ACTOR_EXTERNAL, GET_LARK_IDENTITY_BY_ID, GET_OAR_USER_BY_ID,
    GET_TENANT_BY_ID, UPSERT_LARK_IDENTITY, UPSERT_OAR_USER, UPSERT_TENANT,
};
use oar_core::storage::postgres::operation_ledger_sql::{
    GET_BY_IDEMPOTENCY_KEY, MARK_EXECUTING, MARK_FAILED, MARK_SUCCEEDED,
    SUBMIT_CONFIRMED_ACTION_AND_LEDGER,
};
use oar_core::storage::postgres::token_grant_sql::{
    GET_TOKEN_GRANT_BY_ID, LIST_TOKEN_REFRESH_CANDIDATE_SNAPSHOTS,
    MARK_TOKEN_GRANT_REAUTH_REQUIRED, MARK_TOKEN_GRANT_REFRESH_FAILED, REVOKE_TOKEN_GRANT,
    ROTATE_TOKEN_GRANT, UPSERT_TOKEN_GRANT,
};

fn compact(sql: &str) -> String {
    sql.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

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
fn submit_confirmed_action_and_ledger_uses_tenant_scoped_upsert() {
    let sql = compact(SUBMIT_CONFIRMED_ACTION_AND_LEDGER);

    assert!(sql.contains("insert into confirmed_actions"));
    assert!(sql.contains("insert into operation_ledger"));
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
    assert!(executing.contains("and status = 'confirmed'"));
    assert!(executing.contains("set status = 'executing'"));

    assert!(succeeded.contains("update operation_ledger"));
    assert!(succeeded.contains("and status = 'executing'"));
    assert!(succeeded.contains("set status = 'succeeded'"));

    assert!(failed.contains("update operation_ledger"));
    assert!(failed.contains("and status = 'executing'"));
    assert!(failed.contains("set status = 'failed'"));
    assert!(failed.contains("last_error = $3"));
}

#[test]
fn operation_lookup_is_tenant_scoped() {
    let sql = compact(GET_BY_IDEMPOTENCY_KEY);

    assert!(sql.contains("from operation_ledger"));
    assert!(sql.contains("where tenant_id = $1 and idempotency_key = $2"));
    assert!(sql.contains("limit 1"));
}

#[test]
fn audit_append_only_sql_is_insert_only_and_trace_ordered() {
    let append = compact(APPEND_AUDIT_EVENT);
    let query = compact(FIND_AUDIT_EVENTS_BY_TRACE_ID);

    assert!(append.starts_with("insert into audit_events"));
    assert!(!append.contains(" update "));
    assert!(!append.contains(" delete "));
    assert!(append.contains("operation_id"));
    assert!(append.contains("trace_id"));
    assert!(append.contains("sequence"));

    assert!(query.contains("from audit_events"));
    assert!(query.contains("where trace_id = $1"));
    assert!(query.contains("order by sequence asc"));
}

#[test]
fn audit_outbox_enqueue_records_pending_retry_payload() {
    let sql = compact(ENQUEUE_AUDIT_OUTBOX);

    assert!(sql.starts_with("insert into audit_outbox"));
    assert!(sql.contains("payload"));
    assert!(sql.contains("status"));
    assert!(sql.contains("'pending'"));
    assert!(sql.contains("attempt_count"));
    assert!(sql.contains("returning id"));
}

#[test]
fn audit_outbox_claim_uses_due_pending_rows_with_skip_locked_lease() {
    let sql = compact(CLAIM_AUDIT_OUTBOX);

    assert!(sql.contains("from audit_outbox"));
    assert!(sql.contains("status = 'pending'"));
    assert!(sql.contains("next_attempt_at is null or next_attempt_at <="));
    assert!(sql.contains("for update skip locked"));
    assert!(sql.contains("attempt_count = attempt_count + 1"));
    assert!(sql.contains("next_attempt_at = to_timestamp($5::double precision / 1000.0)"));
}

#[test]
fn audit_outbox_terminal_updates_are_tenant_scoped_and_guarded() {
    let sent = compact(MARK_AUDIT_OUTBOX_SENT);
    let retryable = compact(MARK_AUDIT_OUTBOX_RETRYABLE);
    let failed = compact(MARK_AUDIT_OUTBOX_FAILED);

    assert!(sent.contains("update audit_outbox"));
    assert!(sent.contains("where tenant_id = $1"));
    assert!(sent.contains("and id = $2"));
    assert!(sent.contains("status in ('pending', 'sent')"));
    assert!(sent.contains("set status = 'sent'"));

    assert!(retryable.contains("set status = 'pending'"));
    assert!(retryable.contains("and status = 'pending'"));
    assert!(retryable.contains("next_attempt_at = to_timestamp($3::double precision / 1000.0)"));

    assert!(failed.contains("set status = 'failed'"));
    assert!(failed.contains("and status in ('pending', 'failed')"));
    assert!(failed.contains("next_attempt_at = null"));
}

#[test]
fn audit_outbox_attempt_guarded_updates_bind_current_claim() {
    let sent = compact(MARK_AUDIT_OUTBOX_SENT_FOR_ATTEMPT);
    let retryable = compact(MARK_AUDIT_OUTBOX_RETRYABLE_FOR_ATTEMPT);
    let failed = compact(MARK_AUDIT_OUTBOX_FAILED_FOR_ATTEMPT);

    for sql in [&sent, &retryable, &failed] {
        assert!(sql.contains("where tenant_id = $1"));
        assert!(sql.contains("and id = $2"));
        assert!(sql.contains("and attempt_count = $3"));
        assert!(sql.contains("and next_attempt_at = to_timestamp($4::double precision / 1000.0)"));
        assert!(sql.contains("and status = 'pending'"));
        assert!(sql.contains("returning id"));
    }

    assert!(sent.contains("set status = 'sent'"));
    assert!(
        sent.contains("sent_at = coalesce(sent_at, to_timestamp($5::double precision / 1000.0))")
    );
    assert!(retryable.contains("set status = 'pending'"));
    assert!(retryable.contains("next_attempt_at = to_timestamp($5::double precision / 1000.0)"));
    assert!(failed.contains("set status = 'failed'"));
    assert!(failed.contains("next_attempt_at = null"));
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
    assert!(sql.contains("where tenant_id = $1"));
    assert!(sql.contains("and id = $2"));
    assert!(sql.contains("and oauth_grant_fingerprint = $3"));
    assert!(sql.contains("and state in ('valid', 'needs_refresh', 'expired')"));
    assert!(sql.contains("and revoked_at is null"));
    assert!(sql.contains("and reauth_required_at is null"));
    assert!(sql.contains("returning"));
}

#[test]
fn token_grant_refresh_failure_and_reauth_marks_are_guarded() {
    let refresh_failed = compact(MARK_TOKEN_GRANT_REFRESH_FAILED);
    let reauth_required = compact(MARK_TOKEN_GRANT_REAUTH_REQUIRED);

    for sql in [&refresh_failed, &reauth_required] {
        assert!(sql.contains("update token_grants"));
        assert!(sql.contains("where tenant_id = $1"));
        assert!(sql.contains("and id = $2"));
        assert!(sql.contains("and oauth_grant_fingerprint = $3"));
        assert!(sql.contains("and state in ('valid', 'needs_refresh', 'expired')"));
        assert!(sql.contains("and revoked_at is null"));
        assert!(sql.contains("and reauth_required_at is null"));
        assert!(sql.contains("last_refresh_error = $5"));
        assert!(sql.contains("returning"));
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
fn identity_sql_is_tenant_scoped_and_conflict_guarded() {
    let upsert_tenant = compact(UPSERT_TENANT);
    let get_tenant = compact(GET_TENANT_BY_ID);
    let upsert_user = compact(UPSERT_OAR_USER);
    let get_user = compact(GET_OAR_USER_BY_ID);
    let upsert_identity = compact(UPSERT_LARK_IDENTITY);
    let get_identity = compact(GET_LARK_IDENTITY_BY_ID);
    let get_identity_external = compact(GET_LARK_IDENTITY_BY_ACTOR_EXTERNAL);

    assert!(upsert_tenant.contains("insert into tenants"));
    assert!(upsert_tenant.contains("on conflict (id) do update"));
    assert!(upsert_tenant.contains("status"));
    assert!(get_tenant.contains("from tenants"));
    assert!(get_tenant.contains("where id = $1"));
    assert!(get_tenant.contains("limit 1"));

    assert!(upsert_user.contains("insert into oar_users"));
    assert!(upsert_user.contains("on conflict (id) do update"));
    assert!(upsert_user.contains("where oar_users.tenant_id = excluded.tenant_id"));
    assert!(upsert_user.contains("not exists (select 1 from upserted)"));
    assert!(get_user.contains("from oar_users"));
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

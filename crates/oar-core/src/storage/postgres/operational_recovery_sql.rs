pub const LIST_FAILED_AUDIT_OUTBOX_RECOVERY_ITEMS: &str = r#"
SELECT
    id,
    tenant_id,
    stream,
    aggregate_id,
    payload,
    attempt_count,
    floor(extract(epoch from created_at) * 1000)::bigint AS created_at_ms
FROM audit_outbox
WHERE tenant_id = $1
  AND status = 'failed'
  AND stream = 'audit-events'
  AND sent_at IS NULL
ORDER BY created_at ASC, id ASC
LIMIT $2
"#;

pub const LIST_PARKED_TOKEN_GRANT_RECOVERY_ITEMS: &str = r#"
SELECT
    id,
    tenant_id,
    identity_id,
    actor_kind,
    scope_boundary,
    state,
    last_refresh_error,
    floor(extract(epoch from refreshed_at) * 1000)::bigint AS refreshed_at_ms,
    floor(extract(epoch from reauth_required_at) * 1000)::bigint AS reauth_required_at_ms,
    floor(extract(epoch from updated_at) * 1000)::bigint AS updated_at_ms
FROM token_grants
WHERE tenant_id = $1
  AND (
    state = 'reauth_required'
    OR (
      state IN ('valid', 'needs_refresh', 'expired')
      AND revoked_at IS NULL
      AND reauth_required_at IS NULL
      AND COALESCE(last_refresh_error, '') IN (
        'refresh_config_required',
        'auth_refresh_parse_failed',
        'auth_refresh_oversized_response'
      )
    )
  )
ORDER BY updated_at ASC, id ASC
LIMIT $2
"#;

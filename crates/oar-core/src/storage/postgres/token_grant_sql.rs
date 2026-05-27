pub const TOKEN_GRANT_COLUMNS: &str = r#"
id,
tenant_id,
identity_id,
actor_kind,
scope_boundary,
scopes,
state,
floor(extract(epoch from issued_at) * 1000)::bigint AS issued_at_ms,
floor(extract(epoch from expires_at) * 1000)::bigint AS expires_at_ms,
floor(extract(epoch from refreshed_at) * 1000)::bigint AS refreshed_at_ms,
floor(extract(epoch from revoked_at) * 1000)::bigint AS revoked_at_ms,
floor(extract(epoch from reauth_required_at) * 1000)::bigint AS reauth_required_at_ms,
last_refresh_error,
encrypted_oauth_grant,
oauth_grant_key_id,
oauth_grant_fingerprint,
revocation_reason
"#;

pub const UPSERT_TOKEN_GRANT: &str = r#"
INSERT INTO token_grants (
    id,
    tenant_id,
    identity_id,
    actor_kind,
    scope_boundary,
    scopes,
    state,
    issued_at,
    expires_at,
    refreshed_at,
    revoked_at,
    reauth_required_at,
    last_refresh_error,
    encrypted_oauth_grant,
    oauth_grant_key_id,
    oauth_grant_fingerprint,
    revocation_reason
)
VALUES (
    $1,
    $2,
    $3,
    $4,
    $5,
    $6,
    $7,
    to_timestamp($8::double precision / 1000.0),
    CASE WHEN $9::bigint IS NULL THEN NULL ELSE to_timestamp($9::double precision / 1000.0) END,
    CASE WHEN $10::bigint IS NULL THEN NULL ELSE to_timestamp($10::double precision / 1000.0) END,
    CASE WHEN $11::bigint IS NULL THEN NULL ELSE to_timestamp($11::double precision / 1000.0) END,
    CASE WHEN $12::bigint IS NULL THEN NULL ELSE to_timestamp($12::double precision / 1000.0) END,
    $13,
    $14,
    $15,
    $16,
    $17
)
ON CONFLICT (id) DO UPDATE
SET state = EXCLUDED.state,
    scopes = EXCLUDED.scopes,
    expires_at = EXCLUDED.expires_at,
    refreshed_at = EXCLUDED.refreshed_at,
    revoked_at = EXCLUDED.revoked_at,
    reauth_required_at = EXCLUDED.reauth_required_at,
    last_refresh_error = EXCLUDED.last_refresh_error,
    encrypted_oauth_grant = EXCLUDED.encrypted_oauth_grant,
    oauth_grant_key_id = EXCLUDED.oauth_grant_key_id,
    oauth_grant_fingerprint = EXCLUDED.oauth_grant_fingerprint,
    revocation_reason = EXCLUDED.revocation_reason,
    updated_at = now()
WHERE token_grants.tenant_id = EXCLUDED.tenant_id
RETURNING
id,
tenant_id,
identity_id,
actor_kind,
scope_boundary,
scopes,
state,
floor(extract(epoch from issued_at) * 1000)::bigint AS issued_at_ms,
floor(extract(epoch from expires_at) * 1000)::bigint AS expires_at_ms,
floor(extract(epoch from refreshed_at) * 1000)::bigint AS refreshed_at_ms,
floor(extract(epoch from revoked_at) * 1000)::bigint AS revoked_at_ms,
floor(extract(epoch from reauth_required_at) * 1000)::bigint AS reauth_required_at_ms,
last_refresh_error,
encrypted_oauth_grant,
oauth_grant_key_id,
oauth_grant_fingerprint,
revocation_reason
"#;

pub const GET_TOKEN_GRANT_BY_ID: &str = r#"
SELECT
id,
tenant_id,
identity_id,
actor_kind,
scope_boundary,
scopes,
state,
floor(extract(epoch from issued_at) * 1000)::bigint AS issued_at_ms,
floor(extract(epoch from expires_at) * 1000)::bigint AS expires_at_ms,
floor(extract(epoch from refreshed_at) * 1000)::bigint AS refreshed_at_ms,
floor(extract(epoch from revoked_at) * 1000)::bigint AS revoked_at_ms,
floor(extract(epoch from reauth_required_at) * 1000)::bigint AS reauth_required_at_ms,
last_refresh_error,
encrypted_oauth_grant,
oauth_grant_key_id,
oauth_grant_fingerprint,
revocation_reason
FROM token_grants
WHERE tenant_id = $1
  AND id = $2
LIMIT 1
"#;

pub const ROTATE_TOKEN_GRANT: &str = r#"
UPDATE token_grants
SET state = 'valid',
    expires_at = CASE WHEN $4::bigint IS NULL THEN NULL ELSE to_timestamp($4::double precision / 1000.0) END,
    refreshed_at = to_timestamp($5::double precision / 1000.0),
    last_refresh_error = NULL,
    encrypted_oauth_grant = $6,
    oauth_grant_key_id = $7,
    oauth_grant_fingerprint = $8,
    updated_at = to_timestamp($5::double precision / 1000.0)
WHERE tenant_id = $1
  AND id = $2
  AND oauth_grant_fingerprint = $3
  AND state IN ('valid', 'needs_refresh', 'expired')
  AND revoked_at IS NULL
  AND reauth_required_at IS NULL
RETURNING
id,
tenant_id,
identity_id,
actor_kind,
scope_boundary,
scopes,
state,
floor(extract(epoch from issued_at) * 1000)::bigint AS issued_at_ms,
floor(extract(epoch from expires_at) * 1000)::bigint AS expires_at_ms,
floor(extract(epoch from refreshed_at) * 1000)::bigint AS refreshed_at_ms,
floor(extract(epoch from revoked_at) * 1000)::bigint AS revoked_at_ms,
floor(extract(epoch from reauth_required_at) * 1000)::bigint AS reauth_required_at_ms,
last_refresh_error,
encrypted_oauth_grant,
oauth_grant_key_id,
oauth_grant_fingerprint,
revocation_reason
"#;

pub const MARK_TOKEN_GRANT_REFRESH_FAILED: &str = r#"
UPDATE token_grants
SET state = 'needs_refresh',
    refreshed_at = to_timestamp($4::double precision / 1000.0),
    last_refresh_error = $5,
    updated_at = to_timestamp($4::double precision / 1000.0)
WHERE tenant_id = $1
  AND id = $2
  AND oauth_grant_fingerprint = $3
  AND state IN ('valid', 'needs_refresh', 'expired')
  AND revoked_at IS NULL
  AND reauth_required_at IS NULL
RETURNING
id,
tenant_id,
identity_id,
actor_kind,
scope_boundary,
scopes,
state,
floor(extract(epoch from issued_at) * 1000)::bigint AS issued_at_ms,
floor(extract(epoch from expires_at) * 1000)::bigint AS expires_at_ms,
floor(extract(epoch from refreshed_at) * 1000)::bigint AS refreshed_at_ms,
floor(extract(epoch from revoked_at) * 1000)::bigint AS revoked_at_ms,
floor(extract(epoch from reauth_required_at) * 1000)::bigint AS reauth_required_at_ms,
last_refresh_error,
encrypted_oauth_grant,
oauth_grant_key_id,
oauth_grant_fingerprint,
revocation_reason
"#;

pub const MARK_TOKEN_GRANT_REAUTH_REQUIRED: &str = r#"
UPDATE token_grants
SET state = 'reauth_required',
    reauth_required_at = to_timestamp($4::double precision / 1000.0),
    last_refresh_error = $5,
    updated_at = to_timestamp($4::double precision / 1000.0)
WHERE tenant_id = $1
  AND id = $2
  AND oauth_grant_fingerprint = $3
  AND state IN ('valid', 'needs_refresh', 'expired')
  AND revoked_at IS NULL
  AND reauth_required_at IS NULL
RETURNING
id,
tenant_id,
identity_id,
actor_kind,
scope_boundary,
scopes,
state,
floor(extract(epoch from issued_at) * 1000)::bigint AS issued_at_ms,
floor(extract(epoch from expires_at) * 1000)::bigint AS expires_at_ms,
floor(extract(epoch from refreshed_at) * 1000)::bigint AS refreshed_at_ms,
floor(extract(epoch from revoked_at) * 1000)::bigint AS revoked_at_ms,
floor(extract(epoch from reauth_required_at) * 1000)::bigint AS reauth_required_at_ms,
last_refresh_error,
encrypted_oauth_grant,
oauth_grant_key_id,
oauth_grant_fingerprint,
revocation_reason
"#;

pub const REVOKE_TOKEN_GRANT: &str = r#"
UPDATE token_grants
SET state = 'revoked',
    revoked_at = to_timestamp($3::double precision / 1000.0),
    revocation_reason = $4,
    updated_at = to_timestamp($3::double precision / 1000.0)
WHERE tenant_id = $1
  AND id = $2
  AND state <> 'revoked'
RETURNING
id,
tenant_id,
identity_id,
actor_kind,
scope_boundary,
scopes,
state,
floor(extract(epoch from issued_at) * 1000)::bigint AS issued_at_ms,
floor(extract(epoch from expires_at) * 1000)::bigint AS expires_at_ms,
floor(extract(epoch from refreshed_at) * 1000)::bigint AS refreshed_at_ms,
floor(extract(epoch from revoked_at) * 1000)::bigint AS revoked_at_ms,
floor(extract(epoch from reauth_required_at) * 1000)::bigint AS reauth_required_at_ms,
last_refresh_error,
encrypted_oauth_grant,
oauth_grant_key_id,
oauth_grant_fingerprint,
revocation_reason
"#;

pub const LIST_TOKEN_REFRESH_CANDIDATE_SNAPSHOTS: &str = r#"
SELECT
id,
tenant_id,
oauth_grant_fingerprint,
state,
floor(extract(epoch from revoked_at) * 1000)::bigint AS revoked_at_ms,
floor(extract(epoch from reauth_required_at) * 1000)::bigint AS reauth_required_at_ms,
octet_length(encrypted_oauth_grant) > 0 AS has_refresh_material
FROM token_grants
WHERE tenant_id = $1
  AND state IN ('valid', 'needs_refresh', 'expired')
  AND revoked_at IS NULL
  AND reauth_required_at IS NULL
  AND COALESCE(last_refresh_error, '') NOT IN (
    'refresh_config_required',
    'auth_refresh_parse_failed',
    'auth_refresh_oversized_response'
  )
  AND octet_length(encrypted_oauth_grant) > 0
  AND (
    state IN ('needs_refresh', 'expired')
    OR expires_at <= to_timestamp($2::double precision / 1000.0)
  )
ORDER BY
  CASE WHEN state IN ('needs_refresh', 'expired') THEN 0 ELSE 1 END,
  expires_at ASC NULLS FIRST,
  id ASC
LIMIT $3
"#;

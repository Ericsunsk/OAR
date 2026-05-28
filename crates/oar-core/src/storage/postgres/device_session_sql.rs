pub const UPSERT_DEVICE_SESSION: &str = r#"
WITH upserted AS (
    INSERT INTO device_sessions (
        id,
        tenant_id,
        user_id,
        entry_point,
        state,
        sync_stream,
        sync_cursor_value,
        sync_cursor_updated_at,
        session_identity_hash,
        last_seen_at,
        revoked_at,
        expired_at
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
        $9,
        to_timestamp($10::double precision / 1000.0),
        CASE
            WHEN $11::bigint IS NULL THEN NULL
            ELSE to_timestamp($11::double precision / 1000.0)
        END,
        CASE
            WHEN $12::bigint IS NULL THEN NULL
            ELSE to_timestamp($12::double precision / 1000.0)
        END
    )
    ON CONFLICT (id) DO UPDATE
    SET user_id = EXCLUDED.user_id,
        entry_point = EXCLUDED.entry_point,
        sync_stream = EXCLUDED.sync_stream,
        sync_cursor_value = EXCLUDED.sync_cursor_value,
        sync_cursor_updated_at = EXCLUDED.sync_cursor_updated_at,
        session_identity_hash = EXCLUDED.session_identity_hash,
        last_seen_at = EXCLUDED.last_seen_at,
        updated_at = now()
    WHERE device_sessions.tenant_id = EXCLUDED.tenant_id
      AND device_sessions.state = 'active'
      AND device_sessions.revoked_at IS NULL
      AND device_sessions.expired_at IS NULL
    RETURNING
        id,
        tenant_id,
        user_id,
        entry_point,
        state,
        sync_stream,
        sync_cursor_value,
        floor(extract(epoch from sync_cursor_updated_at) * 1000)::bigint AS sync_cursor_updated_at_ms,
        session_identity_hash,
        floor(extract(epoch from last_seen_at) * 1000)::bigint AS last_seen_at_ms,
        floor(extract(epoch from revoked_at) * 1000)::bigint AS revoked_at_ms,
        floor(extract(epoch from expired_at) * 1000)::bigint AS expired_at_ms
)
SELECT * FROM upserted
UNION ALL
SELECT
    id,
    tenant_id,
    user_id,
    entry_point,
    state,
    sync_stream,
    sync_cursor_value,
    floor(extract(epoch from sync_cursor_updated_at) * 1000)::bigint AS sync_cursor_updated_at_ms,
    session_identity_hash,
    floor(extract(epoch from last_seen_at) * 1000)::bigint AS last_seen_at_ms,
    floor(extract(epoch from revoked_at) * 1000)::bigint AS revoked_at_ms,
    floor(extract(epoch from expired_at) * 1000)::bigint AS expired_at_ms
FROM device_sessions
WHERE tenant_id = $2
  AND id = $1
  AND NOT EXISTS (SELECT 1 FROM upserted)
"#;

pub const GET_DEVICE_SESSION_BY_ID: &str = r#"
SELECT
    id,
    tenant_id,
    user_id,
    entry_point,
    state,
    sync_stream,
    sync_cursor_value,
    floor(extract(epoch from sync_cursor_updated_at) * 1000)::bigint AS sync_cursor_updated_at_ms,
    session_identity_hash,
    floor(extract(epoch from last_seen_at) * 1000)::bigint AS last_seen_at_ms,
    floor(extract(epoch from revoked_at) * 1000)::bigint AS revoked_at_ms,
    floor(extract(epoch from expired_at) * 1000)::bigint AS expired_at_ms
FROM device_sessions
WHERE tenant_id = $1
  AND id = $2
LIMIT 1
"#;

pub const GET_DEVICE_SESSION_BY_SESSION_ID: &str = r#"
SELECT
    id,
    tenant_id,
    user_id,
    entry_point,
    state,
    sync_stream,
    sync_cursor_value,
    floor(extract(epoch from sync_cursor_updated_at) * 1000)::bigint AS sync_cursor_updated_at_ms,
    session_identity_hash,
    floor(extract(epoch from last_seen_at) * 1000)::bigint AS last_seen_at_ms,
    floor(extract(epoch from revoked_at) * 1000)::bigint AS revoked_at_ms,
    floor(extract(epoch from expired_at) * 1000)::bigint AS expired_at_ms
FROM device_sessions
WHERE id = $1
LIMIT 1
"#;

pub const ADVANCE_DEVICE_SESSION_CURSOR_CAS: &str = r#"
UPDATE device_sessions
SET sync_cursor_value = $3,
    sync_cursor_updated_at = to_timestamp($4::double precision / 1000.0),
    last_seen_at = to_timestamp($4::double precision / 1000.0),
    updated_at = now()
WHERE tenant_id = $1
  AND id = $2
  AND sync_cursor_value = $5
  AND $3 > sync_cursor_value
  AND to_timestamp($4::double precision / 1000.0) >= last_seen_at
  AND state = 'active'
  AND revoked_at IS NULL
  AND expired_at IS NULL
RETURNING
    id,
    tenant_id,
    user_id,
    entry_point,
    state,
    sync_stream,
    sync_cursor_value,
    floor(extract(epoch from sync_cursor_updated_at) * 1000)::bigint AS sync_cursor_updated_at_ms,
    session_identity_hash,
    floor(extract(epoch from last_seen_at) * 1000)::bigint AS last_seen_at_ms,
    floor(extract(epoch from revoked_at) * 1000)::bigint AS revoked_at_ms,
    floor(extract(epoch from expired_at) * 1000)::bigint AS expired_at_ms
"#;

pub const REVOKE_DEVICE_SESSION: &str = r#"
UPDATE device_sessions
SET state = 'revoked',
    revoked_at = coalesce(revoked_at, to_timestamp($3::double precision / 1000.0)),
    updated_at = now()
WHERE tenant_id = $1
  AND id = $2
  AND state <> 'revoked'
RETURNING
    id,
    tenant_id,
    user_id,
    entry_point,
    state,
    sync_stream,
    sync_cursor_value,
    floor(extract(epoch from sync_cursor_updated_at) * 1000)::bigint AS sync_cursor_updated_at_ms,
    session_identity_hash,
    floor(extract(epoch from last_seen_at) * 1000)::bigint AS last_seen_at_ms,
    floor(extract(epoch from revoked_at) * 1000)::bigint AS revoked_at_ms,
    floor(extract(epoch from expired_at) * 1000)::bigint AS expired_at_ms
"#;

pub const EXPIRE_DEVICE_SESSION: &str = r#"
UPDATE device_sessions
SET state = 'expired',
    expired_at = coalesce(expired_at, to_timestamp($3::double precision / 1000.0)),
    updated_at = now()
WHERE tenant_id = $1
  AND id = $2
  AND state = 'active'
  AND revoked_at IS NULL
RETURNING
    id,
    tenant_id,
    user_id,
    entry_point,
    state,
    sync_stream,
    sync_cursor_value,
    floor(extract(epoch from sync_cursor_updated_at) * 1000)::bigint AS sync_cursor_updated_at_ms,
    session_identity_hash,
    floor(extract(epoch from last_seen_at) * 1000)::bigint AS last_seen_at_ms,
    floor(extract(epoch from revoked_at) * 1000)::bigint AS revoked_at_ms,
    floor(extract(epoch from expired_at) * 1000)::bigint AS expired_at_ms
"#;

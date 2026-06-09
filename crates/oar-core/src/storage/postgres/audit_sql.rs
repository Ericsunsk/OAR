pub const APPEND_AUDIT_EVENT: &str = r#"
INSERT INTO audit_events (
    event_id,
    trace_id,
    sequence,
    occurred_at_ms,
    tenant_id,
    actor_kind,
    actor_id,
    actor_display_name,
    target_resource_type,
    target_resource_id,
    target_action_type,
    event_type,
    before_summary,
    after_summary,
    execution_result,
    operation_id
)
VALUES (
    $1,
    $2,
    $3,
    $4,
    $5,
    $6,
    $7,
    $8,
    $9,
    $10,
    $11,
    $12,
    $13,
    $14,
    $15,
    $16
)
RETURNING event_id, trace_id, sequence
"#;

pub const FIND_AUDIT_EVENTS_BY_TRACE_ID: &str = r#"
SELECT
    event_id,
    trace_id,
    sequence,
    occurred_at_ms,
    tenant_id,
    actor_kind,
    actor_id,
    actor_display_name,
    target_resource_type,
    target_resource_id,
    target_action_type,
    event_type,
    before_summary,
    after_summary,
    execution_result,
    operation_id
FROM audit_events
WHERE tenant_id = $1
  AND trace_id = $2
ORDER BY sequence ASC
"#;

pub const ENQUEUE_AUDIT_OUTBOX: &str = r#"
INSERT INTO audit_outbox (
    tenant_id,
    stream,
    aggregate_id,
    payload,
    status,
    attempt_count,
    next_attempt_at
)
VALUES ($1, $2, $3, $4, 'pending', 0, to_timestamp($5::double precision / 1000.0))
RETURNING id
"#;

pub const CLAIM_AUDIT_OUTBOX: &str = r#"
WITH candidate AS (
    SELECT id
    FROM audit_outbox
    WHERE tenant_id = $1
      AND stream = $2
      AND status = 'pending'
      AND (next_attempt_at IS NULL OR next_attempt_at <= to_timestamp($3::double precision / 1000.0))
    ORDER BY next_attempt_at NULLS FIRST, created_at ASC, id ASC
    LIMIT $4
    FOR UPDATE SKIP LOCKED
),
claimed AS (
    UPDATE audit_outbox
    SET attempt_count = attempt_count + 1,
        next_attempt_at = to_timestamp($5::double precision / 1000.0)
    WHERE id IN (SELECT id FROM candidate)
    RETURNING id, tenant_id, stream, aggregate_id, payload, status, attempt_count, next_attempt_at
)
SELECT
    id,
    tenant_id,
    stream,
    aggregate_id,
    payload,
    status,
    attempt_count,
    floor(extract(epoch from next_attempt_at) * 1000)::bigint AS next_attempt_at_ms
FROM claimed
ORDER BY id ASC
"#;

pub const MARK_AUDIT_OUTBOX_SENT: &str = r#"
UPDATE audit_outbox
SET status = 'sent',
    sent_at = COALESCE(sent_at, to_timestamp($3::double precision / 1000.0))
WHERE tenant_id = $1
  AND id = $2
  AND status IN ('pending', 'sent')
RETURNING id
"#;

pub const MARK_AUDIT_OUTBOX_SENT_FOR_ATTEMPT: &str = r#"
UPDATE audit_outbox
SET status = 'sent',
    sent_at = COALESCE(sent_at, to_timestamp($5::double precision / 1000.0))
WHERE tenant_id = $1
  AND id = $2
  AND attempt_count = $3
  AND next_attempt_at = to_timestamp($4::double precision / 1000.0)
  AND status = 'pending'
RETURNING id
"#;

pub const MARK_AUDIT_OUTBOX_RETRYABLE: &str = r#"
UPDATE audit_outbox
SET status = 'pending',
    next_attempt_at = to_timestamp($3::double precision / 1000.0)
WHERE tenant_id = $1
  AND id = $2
  AND status = 'pending'
RETURNING id
"#;

pub const MARK_AUDIT_OUTBOX_RETRYABLE_FOR_ATTEMPT: &str = r#"
UPDATE audit_outbox
SET status = 'pending',
    next_attempt_at = to_timestamp($5::double precision / 1000.0)
WHERE tenant_id = $1
  AND id = $2
  AND attempt_count = $3
  AND next_attempt_at = to_timestamp($4::double precision / 1000.0)
  AND status = 'pending'
RETURNING id
"#;

pub const MARK_AUDIT_OUTBOX_FAILED: &str = r#"
UPDATE audit_outbox
SET status = 'failed',
    next_attempt_at = NULL
WHERE tenant_id = $1
  AND id = $2
  AND status IN ('pending', 'failed')
RETURNING id
"#;

pub const MARK_AUDIT_OUTBOX_FAILED_FOR_ATTEMPT: &str = r#"
UPDATE audit_outbox
SET status = 'failed',
    next_attempt_at = NULL
WHERE tenant_id = $1
  AND id = $2
  AND attempt_count = $3
  AND next_attempt_at = to_timestamp($4::double precision / 1000.0)
  AND status = 'pending'
RETURNING id
"#;

pub const LOCK_FAILED_AUDIT_OUTBOX_FOR_RECOVERY: &str = r#"
SELECT
    id,
    payload,
    attempt_count
FROM audit_outbox
WHERE tenant_id = $1
  AND id = $2
  AND attempt_count = $3
  AND stream = 'audit-events'
  AND status = 'failed'
  AND sent_at IS NULL
FOR UPDATE
"#;

pub const REQUEUE_FAILED_AUDIT_OUTBOX_FOR_RECOVERY: &str = r#"
UPDATE audit_outbox
SET status = 'pending',
    next_attempt_at = to_timestamp($4::double precision / 1000.0),
    sent_at = NULL
WHERE tenant_id = $1
  AND id = $2
  AND attempt_count = $3
  AND stream = 'audit-events'
  AND status = 'failed'
  AND sent_at IS NULL
RETURNING id
"#;

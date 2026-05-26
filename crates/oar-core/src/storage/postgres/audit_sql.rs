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
WHERE trace_id = $1
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
VALUES ($1, $2, $3, $4, 'pending', 0, $5)
RETURNING id
"#;

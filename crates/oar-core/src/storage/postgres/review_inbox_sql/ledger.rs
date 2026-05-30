pub const UPDATE_REVIEW_INBOX_LEDGER_PROJECTION: &str = r#"
UPDATE review_inbox_items
SET status = $3,
    ledger_status = $4,
    sync_cursor_value = GREATEST(
        nextval('review_inbox_sync_cursor_seq'),
        sync_cursor_value + 1
    ),
    updated_at = to_timestamp($5::double precision / 1000.0)
WHERE tenant_id = $1
  AND operation_id = $2
  AND status NOT IN ('rejected', 'succeeded', 'failed', 'withdrawn')
  AND COALESCE(ledger_status, 'confirmed') NOT IN ('succeeded', 'failed', 'cancelled')
RETURNING id
"#;

pub const LIST_REVIEW_INBOX_LEDGER_EVENTS_FOR_SNAPSHOT: &str = r#"
WITH selected_items AS (
    SELECT
    tenant_id,
    proposed_action_id,
    proposed_action_version,
    operation_id,
    row_number() OVER (ORDER BY sort_key DESC, updated_at DESC, id ASC) AS item_order
    FROM review_inbox_items
    WHERE tenant_id = $1
      AND user_id = $2
      AND sync_cursor_value > $3
    ORDER BY sort_key DESC, updated_at DESC, id ASC
    LIMIT $4
),
decision_events AS (
    SELECT
    'decision:' || proposed_action_decisions.id AS id,
    selected_items.proposed_action_id AS action_id,
    'confirmed_action' AS stage,
    CASE
        WHEN proposed_action_decisions.decision = 'reject' THEN 'error'
        ELSE 'ok'
    END AS stage_status,
    floor(extract(epoch from proposed_action_decisions.decided_at) * 1000)::bigint AS timestamp_ms,
    CASE
        WHEN proposed_action_decisions.decision = 'reject' THEN 'Review decision rejected.'
        ELSE 'Review decision confirmed.'
    END AS message,
    COALESCE(confirmed_actions.idempotency_key, 'decision:' || proposed_action_decisions.id) AS idempotency_key,
    selected_items.item_order,
    10 AS stage_order
    FROM selected_items
    JOIN proposed_action_decisions
      ON proposed_action_decisions.tenant_id = selected_items.tenant_id
     AND proposed_action_decisions.proposed_action_id = selected_items.proposed_action_id
     AND proposed_action_decisions.proposed_action_version = selected_items.proposed_action_version
    LEFT JOIN confirmed_actions
      ON confirmed_actions.tenant_id = proposed_action_decisions.tenant_id
     AND confirmed_actions.action_id = proposed_action_decisions.confirmed_action_id
),
operation_events AS (
    SELECT
    'operation:' || operation_ledger.operation_id AS id,
    selected_items.proposed_action_id AS action_id,
    'operation_ledger' AS stage,
    CASE
        WHEN operation_ledger.status IN ('failed', 'cancelled') THEN 'error'
        WHEN operation_ledger.status = 'proposed' THEN 'pending'
        ELSE 'ok'
    END AS stage_status,
    floor(extract(epoch from COALESCE(
        operation_ledger.finished_at,
        operation_ledger.executing_at,
        confirmed_actions.confirmed_at,
        operation_ledger.updated_at,
        operation_ledger.created_at
    )) * 1000)::bigint AS timestamp_ms,
    CASE operation_ledger.status
        WHEN 'confirmed' THEN 'Operation ledger confirmed.'
        WHEN 'executing' THEN 'Operation ledger executing.'
        WHEN 'succeeded' THEN 'Operation ledger succeeded.'
        WHEN 'failed' THEN 'Operation ledger failed.'
        WHEN 'cancelled' THEN 'Operation ledger cancelled.'
        ELSE 'Operation ledger pending.'
    END AS message,
    operation_ledger.idempotency_key,
    selected_items.item_order,
    20 AS stage_order
    FROM selected_items
    JOIN operation_ledger
      ON operation_ledger.tenant_id = selected_items.tenant_id
     AND operation_ledger.operation_id = selected_items.operation_id
    JOIN confirmed_actions
      ON confirmed_actions.tenant_id = operation_ledger.tenant_id
     AND confirmed_actions.action_id = operation_ledger.action_id
),
audit_events_for_items AS (
    SELECT
    'audit:' || audit_events.event_id AS id,
    selected_items.proposed_action_id AS action_id,
    'audit_event' AS stage,
    'ok' AS stage_status,
    audit_events.occurred_at_ms AS timestamp_ms,
    CASE audit_events.event_type
        WHEN 'proposed_action_decision_recorded' THEN 'Audit event recorded for review decision.'
        WHEN 'confirmed_action_recorded' THEN 'Audit event recorded for confirmed action.'
        WHEN 'dry_run_executed' THEN 'Audit event recorded for dry run.'
        WHEN 'execution_denied' THEN 'Audit event recorded for execution denial.'
        WHEN 'execution_succeeded' THEN 'Audit event recorded for execution success.'
        WHEN 'execution_failed' THEN 'Audit event recorded for execution failure.'
        ELSE 'Audit event recorded.'
    END AS message,
    COALESCE(operation_ledger.idempotency_key, 'audit:' || audit_events.event_id) AS idempotency_key,
    selected_items.item_order,
    40 AS stage_order
    FROM selected_items
    LEFT JOIN operation_ledger
      ON operation_ledger.tenant_id = selected_items.tenant_id
     AND operation_ledger.operation_id = selected_items.operation_id
    JOIN audit_events
      ON audit_events.tenant_id = selected_items.tenant_id
     AND (
        (
            audit_events.target_resource_type = 'proposed_action'
            AND audit_events.target_resource_id = selected_items.proposed_action_id
        )
        OR (
            operation_ledger.operation_id IS NOT NULL
            AND audit_events.operation_id = operation_ledger.operation_id
        )
     )
),
unioned_events AS (
    SELECT id, action_id, stage, stage_status, timestamp_ms, message, idempotency_key, item_order, stage_order
    FROM decision_events
    UNION ALL
    SELECT id, action_id, stage, stage_status, timestamp_ms, message, idempotency_key, item_order, stage_order
    FROM operation_events
    UNION ALL
    SELECT id, action_id, stage, stage_status, timestamp_ms, message, idempotency_key, item_order, stage_order
    FROM audit_events_for_items
)
SELECT id, action_id, stage, stage_status, timestamp_ms, message, idempotency_key
FROM unioned_events
ORDER BY item_order ASC, timestamp_ms ASC, stage_order ASC, id ASC
"#;

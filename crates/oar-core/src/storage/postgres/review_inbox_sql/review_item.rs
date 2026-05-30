pub const UPSERT_REVIEW_INBOX_ITEM: &str = r#"
INSERT INTO review_inbox_items (
    id,
    tenant_id,
    user_id,
    proposed_action_id,
    proposed_action_version,
    risk_score,
    priority,
    status,
    sort_key,
    source_cursor_value,
    sync_cursor_value,
    updated_at,
    ledger_status,
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
    GREATEST($10, nextval('review_inbox_sync_cursor_seq')),
    to_timestamp($11::double precision / 1000.0),
    $12,
    $13
)
ON CONFLICT (tenant_id, user_id, proposed_action_id) DO UPDATE
SET risk_score = EXCLUDED.risk_score,
    priority = EXCLUDED.priority,
    status = EXCLUDED.status,
    sort_key = EXCLUDED.sort_key,
    source_cursor_value = EXCLUDED.source_cursor_value,
    sync_cursor_value = GREATEST(
        $10,
        nextval('review_inbox_sync_cursor_seq'),
        review_inbox_items.sync_cursor_value + 1
    ),
    updated_at = EXCLUDED.updated_at,
    ledger_status = EXCLUDED.ledger_status,
    operation_id = EXCLUDED.operation_id
WHERE review_inbox_items.source_cursor_value < $10
  AND review_inbox_items.status NOT IN ('rejected', 'succeeded', 'failed', 'withdrawn')
RETURNING id
"#;

pub const LIST_REVIEW_INBOX_ITEMS: &str = r#"
SELECT
id,
tenant_id,
user_id,
proposed_action_id,
proposed_action_version,
risk_score,
priority,
status,
sort_key,
sync_cursor_value,
floor(extract(epoch from updated_at) * 1000)::bigint AS updated_at_ms,
ledger_status,
operation_id
FROM review_inbox_items
WHERE tenant_id = $1
  AND user_id = $2
  AND sync_cursor_value > $3
ORDER BY sort_key DESC, updated_at DESC, id ASC
LIMIT $4
"#;

pub const LOAD_REVIEW_DECISION_ITEM: &str = r#"
SELECT
id,
tenant_id,
user_id,
proposed_action_id,
proposed_action_version,
risk_score,
priority,
status,
sort_key,
sync_cursor_value,
floor(extract(epoch from updated_at) * 1000)::bigint AS updated_at_ms,
ledger_status,
operation_id
FROM review_inbox_items
WHERE tenant_id = $1
  AND user_id = $2
  AND proposed_action_id = $3
  AND proposed_action_version = $4
  AND sync_cursor_value = $5
LIMIT 1
"#;

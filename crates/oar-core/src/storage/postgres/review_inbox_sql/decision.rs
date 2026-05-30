pub const INSERT_PROPOSED_ACTION_DECISION: &str = r#"
INSERT INTO proposed_action_decisions (
    id,
    tenant_id,
    proposed_action_id,
    proposed_action_version,
    actor_user_id,
    decision,
    edited_payload,
    confirmed_action_id,
    decided_at
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
    to_timestamp($9::double precision / 1000.0)
)
ON CONFLICT (tenant_id, proposed_action_id, proposed_action_version) DO NOTHING
RETURNING id
"#;

pub const LOAD_PROPOSED_ACTION_DECISION_FOR_RECORDER: &str = r#"
SELECT
actor_user_id,
decision,
edited_payload,
confirmed_action_id
FROM proposed_action_decisions
WHERE tenant_id = $1
  AND proposed_action_id = $2
  AND proposed_action_version = $3
LIMIT 1
"#;

pub const UPDATE_REVIEW_INBOX_DECISION_STATE: &str = r#"
UPDATE review_inbox_items
SET status = $6,
    sync_cursor_value = GREATEST(
        nextval('review_inbox_sync_cursor_seq'),
        sync_cursor_value + 1
    ),
    updated_at = to_timestamp($7::double precision / 1000.0),
    ledger_status = $8,
    operation_id = $9
WHERE tenant_id = $1
  AND user_id = $2
  AND proposed_action_id = $3
  AND proposed_action_version = $4
  AND sync_cursor_value = $5
  AND status = 'open'
RETURNING id
"#;

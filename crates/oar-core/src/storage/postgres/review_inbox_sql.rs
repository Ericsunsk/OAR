pub const INSERT_EVIDENCE_ITEM: &str = r#"
INSERT INTO evidence_items (
    id,
    tenant_id,
    summary,
    source_kind,
    source_id,
    locator,
    content_hash,
    visibility_scope,
    observed_at,
    recorded_at
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
    to_timestamp($9::double precision / 1000.0),
    to_timestamp($10::double precision / 1000.0)
)
ON CONFLICT (tenant_id, id) DO NOTHING
RETURNING
id,
tenant_id,
summary,
source_kind,
source_id,
locator,
content_hash,
visibility_scope,
floor(extract(epoch from observed_at) * 1000)::bigint AS observed_at_ms,
floor(extract(epoch from recorded_at) * 1000)::bigint AS recorded_at_ms
"#;

pub const INSERT_PROPOSED_ACTION: &str = r#"
INSERT INTO proposed_actions (
    id,
    tenant_id,
    actor_user_id,
    target_user_id,
    owner_user_id,
    version,
    status,
    kind,
    custom_kind,
    risk_severity,
    suggested_payload,
    published_at
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
    CASE WHEN $12::bigint IS NULL THEN NULL ELSE to_timestamp($12::double precision / 1000.0) END
)
ON CONFLICT (tenant_id, id, version) DO NOTHING
RETURNING id
"#;

pub const INSERT_PROPOSED_ACTION_EVIDENCE_REF: &str = r#"
INSERT INTO proposed_action_evidence_refs (
    proposed_action_id,
    evidence_id,
    tenant_id,
    proposed_action_version
)
VALUES ($1, $2, $3, $4)
ON CONFLICT (tenant_id, proposed_action_id, proposed_action_version, evidence_id) DO NOTHING
"#;

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

pub const UPDATE_REVIEW_INBOX_LEDGER_PROJECTION: &str = r#"
UPDATE review_inbox_items
SET status = $3,
    ledger_status = $4,
    source_cursor_value = GREATEST(source_cursor_value, $5),
    sync_cursor_value = GREATEST(
        $5,
        nextval('review_inbox_sync_cursor_seq'),
        sync_cursor_value + 1
    ),
    updated_at = to_timestamp($6::double precision / 1000.0)
WHERE tenant_id = $1
  AND operation_id = $2
  AND status NOT IN ('rejected', 'succeeded', 'failed', 'withdrawn')
  AND COALESCE(ledger_status, 'confirmed') NOT IN ('succeeded', 'failed', 'cancelled')
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

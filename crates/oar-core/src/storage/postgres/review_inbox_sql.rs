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

pub const LIST_REVIEW_INBOX_ACTIONS_FOR_SNAPSHOT: &str = r#"
WITH selected_items AS (
    SELECT
    id,
    tenant_id,
    proposed_action_id,
    proposed_action_version,
    sort_key,
    updated_at
    FROM review_inbox_items
    WHERE tenant_id = $1
      AND user_id = $2
      AND sync_cursor_value > $3
    ORDER BY sort_key DESC, updated_at DESC, id ASC
    LIMIT $4
)
SELECT
selected_items.id AS review_item_id,
proposed_actions.id,
proposed_actions.tenant_id,
proposed_actions.actor_user_id,
proposed_actions.target_user_id,
proposed_actions.owner_user_id,
proposed_actions.version,
proposed_actions.status,
proposed_actions.kind,
proposed_actions.custom_kind,
proposed_actions.risk_severity,
proposed_actions.suggested_payload,
COALESCE(
    array_agg(proposed_action_evidence_refs.evidence_id ORDER BY proposed_action_evidence_refs.evidence_id)
        FILTER (WHERE proposed_action_evidence_refs.evidence_id IS NOT NULL),
    ARRAY[]::text[]
) AS evidence_ids,
proposed_action_decisions.id AS decision_id,
proposed_action_decisions.actor_user_id AS decision_actor_user_id,
proposed_action_decisions.decision,
proposed_action_decisions.confirmed_action_id,
floor(extract(epoch from proposed_action_decisions.decided_at) * 1000)::bigint AS decided_at_ms
FROM selected_items
JOIN proposed_actions
  ON proposed_actions.tenant_id = selected_items.tenant_id
 AND proposed_actions.id = selected_items.proposed_action_id
 AND proposed_actions.version = selected_items.proposed_action_version
LEFT JOIN proposed_action_evidence_refs
  ON proposed_action_evidence_refs.tenant_id = proposed_actions.tenant_id
 AND proposed_action_evidence_refs.proposed_action_id = proposed_actions.id
 AND proposed_action_evidence_refs.proposed_action_version = proposed_actions.version
LEFT JOIN proposed_action_decisions
  ON proposed_action_decisions.tenant_id = proposed_actions.tenant_id
 AND proposed_action_decisions.proposed_action_id = proposed_actions.id
 AND proposed_action_decisions.proposed_action_version = proposed_actions.version
GROUP BY
selected_items.id,
selected_items.sort_key,
selected_items.updated_at,
proposed_actions.id,
proposed_actions.tenant_id,
proposed_actions.actor_user_id,
proposed_actions.target_user_id,
proposed_actions.owner_user_id,
proposed_actions.version,
proposed_actions.status,
proposed_actions.kind,
proposed_actions.custom_kind,
proposed_actions.risk_severity,
proposed_actions.suggested_payload,
proposed_action_decisions.id,
proposed_action_decisions.actor_user_id,
proposed_action_decisions.decision,
proposed_action_decisions.confirmed_action_id,
proposed_action_decisions.decided_at
ORDER BY selected_items.sort_key DESC, selected_items.updated_at DESC, selected_items.id ASC
"#;

pub const LIST_REVIEW_INBOX_EVIDENCE_FOR_SNAPSHOT: &str = r#"
WITH selected_items AS (
    SELECT
    id,
    tenant_id,
    proposed_action_id,
    proposed_action_version,
    sort_key,
    updated_at
    FROM review_inbox_items
    WHERE tenant_id = $1
      AND user_id = $2
      AND sync_cursor_value > $3
    ORDER BY sort_key DESC, updated_at DESC, id ASC
    LIMIT $4
)
SELECT
selected_items.id AS review_item_id,
evidence_items.id,
evidence_items.tenant_id,
evidence_items.summary,
evidence_items.source_kind,
evidence_items.source_id,
evidence_items.locator,
evidence_items.content_hash,
evidence_items.visibility_scope,
floor(extract(epoch from evidence_items.observed_at) * 1000)::bigint AS observed_at_ms,
floor(extract(epoch from evidence_items.recorded_at) * 1000)::bigint AS recorded_at_ms
FROM selected_items
JOIN proposed_action_evidence_refs
  ON proposed_action_evidence_refs.tenant_id = selected_items.tenant_id
 AND proposed_action_evidence_refs.proposed_action_id = selected_items.proposed_action_id
 AND proposed_action_evidence_refs.proposed_action_version = selected_items.proposed_action_version
JOIN evidence_items
  ON evidence_items.tenant_id = proposed_action_evidence_refs.tenant_id
 AND evidence_items.id = proposed_action_evidence_refs.evidence_id
ORDER BY
selected_items.sort_key DESC,
selected_items.updated_at DESC,
selected_items.id ASC,
evidence_items.observed_at DESC,
evidence_items.id ASC
"#;

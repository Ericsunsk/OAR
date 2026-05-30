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

pub const LOAD_REVIEW_DECISION_EVIDENCE: &str = r#"
SELECT
review_inbox_items.id AS review_item_id,
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
FROM review_inbox_items
JOIN proposed_action_evidence_refs
  ON proposed_action_evidence_refs.tenant_id = review_inbox_items.tenant_id
 AND proposed_action_evidence_refs.proposed_action_id = review_inbox_items.proposed_action_id
 AND proposed_action_evidence_refs.proposed_action_version = review_inbox_items.proposed_action_version
JOIN evidence_items
  ON evidence_items.tenant_id = proposed_action_evidence_refs.tenant_id
 AND evidence_items.id = proposed_action_evidence_refs.evidence_id
WHERE review_inbox_items.tenant_id = $1
  AND review_inbox_items.user_id = $2
  AND review_inbox_items.proposed_action_id = $3
  AND review_inbox_items.proposed_action_version = $4
  AND review_inbox_items.sync_cursor_value = $5
ORDER BY evidence_items.observed_at DESC, evidence_items.id ASC
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

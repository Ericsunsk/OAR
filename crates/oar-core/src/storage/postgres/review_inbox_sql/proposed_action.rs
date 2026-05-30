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

pub const LOAD_REVIEW_DECISION_ACTION: &str = r#"
SELECT
review_inbox_items.id AS review_item_id,
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
FROM review_inbox_items
JOIN proposed_actions
  ON proposed_actions.tenant_id = review_inbox_items.tenant_id
 AND proposed_actions.id = review_inbox_items.proposed_action_id
 AND proposed_actions.version = review_inbox_items.proposed_action_version
LEFT JOIN proposed_action_evidence_refs
  ON proposed_action_evidence_refs.tenant_id = proposed_actions.tenant_id
 AND proposed_action_evidence_refs.proposed_action_id = proposed_actions.id
 AND proposed_action_evidence_refs.proposed_action_version = proposed_actions.version
LEFT JOIN proposed_action_decisions
  ON proposed_action_decisions.tenant_id = proposed_actions.tenant_id
 AND proposed_action_decisions.proposed_action_id = proposed_actions.id
 AND proposed_action_decisions.proposed_action_version = proposed_actions.version
WHERE review_inbox_items.tenant_id = $1
  AND review_inbox_items.user_id = $2
  AND review_inbox_items.proposed_action_id = $3
  AND review_inbox_items.proposed_action_version = $4
  AND review_inbox_items.sync_cursor_value = $5
GROUP BY
review_inbox_items.id,
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
LIMIT 1
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

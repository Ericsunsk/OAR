use std::collections::HashMap;
use std::time::SystemTime;

use oar_core::storage::postgres::{
    StoredReviewInboxAction, StoredReviewInboxEvidence, StoredReviewInboxItem,
    StoredReviewInboxLedgerEvent, StoredReviewInboxSnapshot,
};
use serde_json::Value;

use crate::feishu_auth::iso8601_utc;

use super::dto::{
    review_item_status, EvidenceItemDto, LedgerEventDto, ProposedActionDto, ReviewInboxItemDto,
    ReviewInboxSnapshotDto,
};
use super::labels::action_status;

mod labels;
mod safe_text;

use labels::{
    default_signal_type, evidence_source_kind, evidence_visibility, ledger_stage, ledger_status,
    proposed_action_decision, proposed_action_kind, proposed_action_status, risk_severity,
    signal_type,
};
use safe_text::{safe_correlation_key, safe_ledger_text, sanitized_text_or};

const CONTRACT_VERSION: u64 = 1;

pub(super) fn snapshot_response_body(
    snapshot: &StoredReviewInboxSnapshot,
    generated_at: SystemTime,
) -> Value {
    serde_json::to_value(snapshot_dto(snapshot, generated_at))
        .expect("review inbox snapshot dto is serializable")
}

fn snapshot_dto(
    snapshot: &StoredReviewInboxSnapshot,
    generated_at: SystemTime,
) -> ReviewInboxSnapshotDto {
    let actions_by_item = actions_by_review_item(&snapshot.actions);
    let evidence_by_item = evidence_by_review_item(&snapshot.evidence);

    ReviewInboxSnapshotDto {
        contract_version: CONTRACT_VERSION,
        generated_at: iso8601_utc(generated_at),
        items: snapshot
            .items
            .iter()
            .map(|item| {
                item_dto(
                    item,
                    actions_by_item.get(item.id.as_str()).copied(),
                    evidence_by_item.get(item.id.as_str()).map(Vec::as_slice),
                )
            })
            .collect(),
        proposed_actions: snapshot.actions.iter().map(proposed_action_dto).collect(),
        evidence: snapshot
            .evidence
            .iter()
            .map(|evidence| {
                evidence_dto(
                    evidence,
                    actions_by_item
                        .get(evidence.review_item_id.as_str())
                        .copied()
                        .map(|action| &action.suggested_payload),
                )
            })
            .collect(),
        ledger_events: snapshot
            .ledger_events
            .iter()
            .map(ledger_event_dto)
            .collect(),
    }
}

fn actions_by_review_item(
    actions: &[StoredReviewInboxAction],
) -> HashMap<&str, &StoredReviewInboxAction> {
    actions
        .iter()
        .map(|action| (action.review_item_id.as_str(), action))
        .collect()
}

fn evidence_by_review_item(
    evidence: &[StoredReviewInboxEvidence],
) -> HashMap<&str, Vec<&StoredReviewInboxEvidence>> {
    let mut by_item: HashMap<&str, Vec<&StoredReviewInboxEvidence>> = HashMap::new();
    for evidence in evidence {
        by_item
            .entry(evidence.review_item_id.as_str())
            .or_default()
            .push(evidence);
    }
    by_item
}

fn item_dto(
    item: &StoredReviewInboxItem,
    action: Option<&StoredReviewInboxAction>,
    evidence: Option<&[&StoredReviewInboxEvidence]>,
) -> ReviewInboxItemDto {
    let payload = action.map(|action| &action.suggested_payload);
    ReviewInboxItemDto {
        id: item.id.clone(),
        tenant_id: item.tenant_id.clone(),
        user_id: item.user_id.clone(),
        proposed_action_id: item.proposed_action_id.clone(),
        proposed_action_version: item.proposed_action_version,
        objective_title: whitelisted_payload_string(payload, "objective_title")
            .unwrap_or_else(|| format!("Review item {}", item.id)),
        key_result_title: whitelisted_payload_string(payload, "key_result_title")
            .or_else(|| first_evidence_summary(evidence))
            .unwrap_or_else(|| "Review required".to_string()),
        owner_display_name: whitelisted_payload_string(payload, "owner_display_name")
            .or_else(|| action.and_then(|action| action.owner_user_id.clone()))
            .unwrap_or_else(|| "Unassigned".to_string()),
        week_label: whitelisted_payload_string(payload, "week_label")
            .unwrap_or_else(|| "Current week".to_string()),
        risk_score: item.risk_score,
        priority: item.priority,
        risk_reason: whitelisted_payload_string(payload, "risk_reason")
            .or_else(|| first_evidence_summary(evidence))
            .unwrap_or_else(|| "Review required before any platform write.".to_string()),
        confidence_score: payload_number(payload, "confidence_score")
            .unwrap_or_else(|| f64::from(item.risk_score.min(100)) / 100.0)
            .clamp(0.0, 1.0),
        status: review_item_status(item.status),
        sync_cursor: item.sync_cursor_value,
        updated_at_display: iso8601_utc(item.updated_at),
        ledger_status: item.ledger_status.map(action_status),
        operation_id: item.operation_id.clone(),
    }
}

fn proposed_action_dto(action: &StoredReviewInboxAction) -> ProposedActionDto {
    let payload = Some(&action.suggested_payload);
    ProposedActionDto {
        id: action.id.clone(),
        review_item_id: action.review_item_id.clone(),
        tenant_id: action.tenant_id.clone(),
        actor_user_id: action.actor_user_id.clone(),
        target_user_id: action.target_user_id.clone(),
        owner_user_id: action.owner_user_id.clone(),
        version: action.version,
        status: proposed_action_status(action.status),
        kind: proposed_action_kind(&action.kind),
        risk_severity: risk_severity(action.risk_severity),
        evidence_ids: action.evidence_ids.clone(),
        rationale: whitelisted_payload_string(payload, "rationale")
            .unwrap_or_else(|| format!("Review proposed action {} before execution.", action.id)),
        expected_impact: whitelisted_payload_string(payload, "expected_impact")
            .unwrap_or_else(|| "No production write will occur before confirmation.".to_string()),
        dry_run_result_summary: whitelisted_payload_string(payload, "dry_run_result_summary")
            .unwrap_or_else(|| "Dry-run summary unavailable.".to_string()),
        estimated_write_targets_count: payload_integer(payload, "estimated_write_targets_count")
            .unwrap_or(0),
        decision: action
            .decision
            .as_ref()
            .map(|decision| proposed_action_decision(decision.decision)),
    }
}

fn evidence_dto(
    evidence: &StoredReviewInboxEvidence,
    action_payload: Option<&Value>,
) -> EvidenceItemDto {
    EvidenceItemDto {
        id: evidence.item.id.clone(),
        review_item_id: evidence.review_item_id.clone(),
        source_kind: evidence_source_kind(evidence.item.source_kind),
        source_id: evidence.item.source_id.clone(),
        locator: evidence.item.locator.clone(),
        observed_at_display: iso8601_utc(evidence.item.observed_at),
        summary: sanitized_text_or(Some(evidence.item.summary.as_str()), "Evidence unavailable"),
        signal_type: whitelisted_payload_string(action_payload, "signal_type")
            .as_deref()
            .map(signal_type)
            .unwrap_or_else(|| default_signal_type(evidence.item.source_kind)),
        trust_score: payload_number(action_payload, "trust_score")
            .unwrap_or(0.7)
            .clamp(0.0, 1.0),
        content_hash: evidence.item.content_hash.clone(),
        visibility: evidence_visibility(evidence.item.visibility_scope),
    }
}

fn ledger_event_dto(event: &StoredReviewInboxLedgerEvent) -> LedgerEventDto {
    LedgerEventDto {
        id: event.id.clone(),
        action_id: event.action_id.clone(),
        stage: ledger_stage(event.stage),
        stage_status: ledger_status(event.stage_status),
        timestamp_display: iso8601_utc(event.timestamp),
        message: safe_ledger_text(event.message.as_str(), "Ledger event recorded."),
        idempotency_key: safe_correlation_key(event.idempotency_key.as_str()),
    }
}

fn first_evidence_summary(evidence: Option<&[&StoredReviewInboxEvidence]>) -> Option<String> {
    evidence
        .and_then(|items| items.first())
        .map(|evidence| sanitized_text_or(Some(evidence.item.summary.as_str()), ""))
        .filter(|value| !value.is_empty())
}

fn whitelisted_payload_string(payload: Option<&Value>, key: &str) -> Option<String> {
    payload
        .and_then(|value| value.get(key))
        .and_then(Value::as_str)
        .map(|value| sanitized_text_or(Some(value), ""))
        .filter(|value| !value.is_empty())
}

fn payload_number(payload: Option<&Value>, key: &str) -> Option<f64> {
    payload
        .and_then(|value| value.get(key))
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
}

fn payload_integer(payload: Option<&Value>, key: &str) -> Option<u64> {
    payload
        .and_then(|value| value.get(key))
        .and_then(Value::as_u64)
}

use oar_core::domain::review_inbox::ReviewInboxItemStatus;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(super) struct ReviewInboxSnapshotDto {
    pub(super) contract_version: u64,
    pub(super) generated_at: String,
    pub(super) items: Vec<ReviewInboxItemDto>,
    pub(super) proposed_actions: Vec<ProposedActionDto>,
    pub(super) evidence: Vec<EvidenceItemDto>,
    pub(super) ledger_events: Vec<LedgerEventDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(super) struct ReviewInboxItemDto {
    pub(super) id: String,
    pub(super) tenant_id: String,
    pub(super) user_id: String,
    pub(super) proposed_action_id: String,
    pub(super) proposed_action_version: u64,
    pub(super) objective_title: String,
    pub(super) key_result_title: String,
    pub(super) owner_display_name: String,
    pub(super) week_label: String,
    pub(super) risk_score: u32,
    pub(super) priority: u32,
    pub(super) risk_reason: String,
    pub(super) confidence_score: f64,
    pub(super) status: &'static str,
    pub(super) sync_cursor: u64,
    pub(super) updated_at_display: String,
    pub(super) ledger_status: Option<&'static str>,
    pub(super) operation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(super) struct ProposedActionDto {
    pub(super) id: String,
    pub(super) review_item_id: String,
    pub(super) tenant_id: String,
    pub(super) actor_user_id: String,
    pub(super) target_user_id: Option<String>,
    pub(super) owner_user_id: Option<String>,
    pub(super) version: u64,
    pub(super) status: &'static str,
    pub(super) kind: String,
    pub(super) risk_severity: &'static str,
    pub(super) evidence_ids: Vec<String>,
    pub(super) rationale: String,
    pub(super) expected_impact: String,
    pub(super) dry_run_result_summary: String,
    pub(super) estimated_write_targets_count: u64,
    pub(super) decision: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(super) struct EvidenceItemDto {
    pub(super) id: String,
    pub(super) review_item_id: String,
    pub(super) source_kind: &'static str,
    pub(super) source_id: String,
    pub(super) locator: Option<String>,
    pub(super) observed_at_display: String,
    pub(super) summary: String,
    pub(super) signal_type: &'static str,
    pub(super) trust_score: f64,
    pub(super) content_hash: String,
    pub(super) visibility: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(super) struct LedgerEventDto {
    pub(super) id: String,
    pub(super) action_id: String,
    pub(super) stage: &'static str,
    pub(super) stage_status: &'static str,
    pub(super) timestamp_display: String,
    pub(super) message: String,
    pub(super) idempotency_key: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub(super) struct ReviewDecisionRequestDto {
    #[serde(rename = "action_id")]
    pub(super) action_id: String,
    #[serde(rename = "action_version")]
    pub(super) action_version: u64,
    pub(super) decision: ReviewDecisionKindDto,
    pub(super) note: String,
    #[serde(rename = "expected_sync_cursor")]
    pub(super) expected_sync_cursor: Option<u64>,
    #[serde(default, rename = "edited_payload")]
    pub(super) edited_payload: Option<Value>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum ReviewDecisionKindDto {
    Confirm,
    EditThenConfirm,
    Reject,
}

pub(super) fn review_item_status(status: ReviewInboxItemStatus) -> &'static str {
    match status {
        ReviewInboxItemStatus::Open => "open",
        ReviewInboxItemStatus::Confirmed => "confirmed",
        ReviewInboxItemStatus::Rejected => "rejected",
        ReviewInboxItemStatus::Executing => "executing",
        ReviewInboxItemStatus::Succeeded => "succeeded",
        ReviewInboxItemStatus::Failed => "failed",
        ReviewInboxItemStatus::Withdrawn => "withdrawn",
    }
}

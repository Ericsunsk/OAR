use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredEvidenceItem {
    pub id: String,
    pub tenant_id: String,
    pub summary: String,
    pub source_kind: EvidenceSourceKind,
    pub source_id: String,
    pub locator: Option<String>,
    pub content_hash: String,
    pub visibility_scope: EvidenceVisibilityScope,
    pub observed_at: SystemTime,
    pub recorded_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoredProposedAction {
    pub id: String,
    pub tenant_id: String,
    pub actor_user_id: String,
    pub target_user_id: Option<String>,
    pub owner_user_id: Option<String>,
    pub version: u64,
    pub status: ProposedActionStatus,
    pub kind: ProposedActionKind,
    pub risk_severity: RiskSeverity,
    pub suggested_payload: Value,
    pub published_at: Option<SystemTime>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoredProposedActionDecision {
    pub id: String,
    pub tenant_id: String,
    pub proposed_action_id: String,
    pub proposed_action_version: u64,
    pub actor_user_id: String,
    pub decision: ProposedActionDecision,
    pub confirmed_action_id: Option<String>,
    pub decided_at: SystemTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoredProposedActionDecisionKind {
    Confirm,
    EditThenConfirm,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredReviewInboxItem {
    pub id: String,
    pub tenant_id: String,
    pub user_id: String,
    pub proposed_action_id: String,
    pub proposed_action_version: u64,
    pub risk_score: u32,
    pub priority: u32,
    pub status: ReviewInboxItemStatus,
    pub sort_key: i64,
    pub sync_cursor_value: u64,
    pub updated_at: SystemTime,
    pub ledger_status: Option<ActionStatus>,
    pub operation_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoredReviewInboxSnapshot {
    pub items: Vec<StoredReviewInboxItem>,
    pub actions: Vec<StoredReviewInboxAction>,
    pub evidence: Vec<StoredReviewInboxEvidence>,
    pub ledger_events: Vec<StoredReviewInboxLedgerEvent>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoredReviewDecisionContext {
    pub item: StoredReviewInboxItem,
    pub action: StoredReviewInboxAction,
    pub evidence: Vec<StoredReviewInboxEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostgresReviewDecisionContextRequest<'a> {
    pub tenant_id: &'a str,
    pub user_id: &'a str,
    pub proposed_action_id: &'a str,
    pub proposed_action_version: u64,
    pub expected_sync_cursor_value: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoredReviewInboxAction {
    pub review_item_id: String,
    pub id: String,
    pub tenant_id: String,
    pub actor_user_id: String,
    pub target_user_id: Option<String>,
    pub owner_user_id: Option<String>,
    pub version: u64,
    pub status: ProposedActionStatus,
    pub kind: ProposedActionKind,
    pub risk_severity: RiskSeverity,
    pub evidence_ids: Vec<String>,
    pub suggested_payload: Value,
    pub decision: Option<StoredReviewInboxActionDecision>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredReviewInboxActionDecision {
    pub id: String,
    pub actor_user_id: String,
    pub decision: StoredProposedActionDecisionKind,
    pub confirmed_action_id: Option<String>,
    pub decided_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredReviewInboxEvidence {
    pub review_item_id: String,
    pub item: StoredEvidenceItem,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredReviewInboxLedgerEvent {
    pub id: String,
    pub action_id: String,
    pub stage: StoredReviewInboxLedgerStage,
    pub stage_status: StoredReviewInboxLedgerStatus,
    pub timestamp: SystemTime,
    pub message: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoredReviewInboxLedgerStage {
    ConfirmedAction,
    OperationLedger,
    PlatformAdapter,
    AuditEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoredReviewInboxLedgerStatus {
    Pending,
    Ok,
    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InsertProposedActionDecisionRequest<'a> {
    pub id: &'a str,
    pub tenant_id: &'a str,
    pub proposed_action_id: &'a str,
    pub proposed_action_version: u64,
    pub actor_user_id: &'a str,
    pub decision: &'a ProposedActionDecision,
    pub confirmed_action_id: Option<&'a str>,
    pub decided_at: SystemTime,
}

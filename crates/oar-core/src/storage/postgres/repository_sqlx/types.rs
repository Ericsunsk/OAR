use super::*;

#[derive(Clone, PartialEq, Eq)]
pub struct EncryptedTokenGrantRecord {
    pub id: String,
    pub tenant_id: String,
    pub identity_id: String,
    pub actor_kind: ActorKind,
    pub scope_boundary: ScopeBoundary,
    pub scopes: Vec<String>,
    pub state: TokenGrantState,
    pub issued_at_ms: u64,
    pub expires_at_ms: Option<u64>,
    pub refreshed_at_ms: Option<u64>,
    pub revoked_at_ms: Option<u64>,
    pub reauth_required_at_ms: Option<u64>,
    pub last_refresh_error: Option<String>,
    pub encrypted_oauth_grant: Vec<u8>,
    pub oauth_grant_key_id: String,
    pub oauth_grant_fingerprint: String,
    pub revocation_reason: Option<String>,
}

impl fmt::Debug for EncryptedTokenGrantRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EncryptedTokenGrantRecord")
            .field("id", &self.id)
            .field("tenant_id", &self.tenant_id)
            .field("identity_id", &self.identity_id)
            .field("actor_kind", &self.actor_kind)
            .field("scope_boundary", &self.scope_boundary)
            .field("scopes", &self.scopes)
            .field("state", &self.state)
            .field("issued_at_ms", &self.issued_at_ms)
            .field("expires_at_ms", &self.expires_at_ms)
            .field("refreshed_at_ms", &self.refreshed_at_ms)
            .field("revoked_at_ms", &self.revoked_at_ms)
            .field("reauth_required_at_ms", &self.reauth_required_at_ms)
            .field("last_refresh_error", &self.last_refresh_error)
            .field(
                "encrypted_oauth_grant",
                &format_args!("[REDACTED; bytes={}]", self.encrypted_oauth_grant.len()),
            )
            .field("oauth_grant_key_id", &"[REDACTED]")
            .field("oauth_grant_fingerprint", &"[REDACTED]")
            .field("revocation_reason", &self.revocation_reason)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct RotateEncryptedGrantRequest<'a> {
    pub tenant_id: &'a str,
    pub id: &'a str,
    pub expected_fingerprint: &'a str,
    pub expires_at_ms: Option<u64>,
    pub refreshed_at_ms: u64,
    pub encrypted_oauth_grant: &'a [u8],
    pub oauth_grant_key_id: &'a str,
    pub oauth_grant_fingerprint: &'a str,
}

impl fmt::Debug for RotateEncryptedGrantRequest<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RotateEncryptedGrantRequest")
            .field("tenant_id", &self.tenant_id)
            .field("id", &self.id)
            .field("expected_fingerprint", &"[REDACTED]")
            .field("expires_at_ms", &self.expires_at_ms)
            .field("refreshed_at_ms", &self.refreshed_at_ms)
            .field(
                "encrypted_oauth_grant",
                &format_args!("[REDACTED; bytes={}]", self.encrypted_oauth_grant.len()),
            )
            .field("oauth_grant_key_id", &"[REDACTED]")
            .field("oauth_grant_fingerprint", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredTenant {
    pub id: String,
    pub display_name: String,
    pub status: TenantStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredWorkspaceUser {
    pub id: String,
    pub tenant_id: String,
    pub display_name: String,
    pub status: WorkspaceUserStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredLarkIdentity {
    pub id: String,
    pub tenant_id: String,
    pub actor_kind: ActorKind,
    pub actor_external_id: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredDeviceSession {
    pub id: String,
    pub tenant_id: String,
    pub user_id: String,
    pub entry_point: crate::domain::device_sync::DeviceEntryPoint,
    pub state: crate::domain::device_sync::SessionState,
    pub sync_stream: String,
    pub sync_cursor_value: u64,
    pub sync_cursor_updated_at: SystemTime,
    pub session_identity_hash: String,
    pub last_seen_at: SystemTime,
    pub revoked_at: Option<SystemTime>,
    pub expired_at: Option<SystemTime>,
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredPendingConfirmedAction {
    pub action: ConfirmedAction,
    pub operation: OperationRecord,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSchedulerJob {
    pub id: String,
    pub tenant_id: String,
    pub job_kind: SchedulerJobKind,
    pub status: SchedulerJobStatus,
    pub next_run_at_ms: u64,
    pub lease_id: Option<String>,
    pub lease_until_ms: Option<u64>,
    pub attempt_count: u32,
    pub last_started_at_ms: Option<u64>,
    pub last_finished_at_ms: Option<u64>,
    pub last_safe_error_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuditOutboxMessage {
    pub id: i64,
    pub tenant_id: String,
    pub stream: String,
    pub aggregate_id: String,
    pub payload: Value,
    pub attempt_count: i32,
    pub next_attempt_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuditOutboxEnvelope {
    pub tenant_id: String,
    pub stream: String,
    pub aggregate_id: String,
    pub payload: Value,
    pub next_attempt_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PostgresExecutionRecorderReport {
    pub operation: OperationRecord,
    pub outbox_id: Option<i64>,
    pub inbox_item_id: Option<String>,
    pub duplicate: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PostgresReviewDecisionRecorderReport {
    pub operation: Option<OperationRecord>,
    pub inbox_item_id: Option<String>,
    pub outbox_id: Option<i64>,
    pub duplicate: bool,
}

#[derive(Debug, Clone)]
pub struct PostgresReviewDecisionRecorderRequest<'a> {
    pub expected_sync_cursor_value: u64,
    pub decision: InsertProposedActionDecisionRequest<'a>,
    pub confirmed_action: Option<&'a ConfirmedAction>,
    pub confirmed_at_ms: Option<u64>,
    pub operation_id: Option<&'a str>,
    pub inbox_item: &'a ReviewInboxItem,
    pub event: &'a AuditEvent,
    pub outbox: &'a AuditOutboxEnvelope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostgresTokenRefreshRecorderReport {
    pub apply_result: Option<TokenRefreshApplyResult>,
    pub event: AuditEvent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostgresTokenRefreshOrchestratorReport {
    pub service_report: TokenRefreshServiceReport,
    pub event: AuditEvent,
}

#[derive(Clone)]
pub struct PostgresTokenRefreshSweepRequest {
    pub tenant_id: String,
    pub due_before: SystemTime,
    pub limit: u32,
    pub now: SystemTime,
    pub audit_trace_id: String,
    pub audit_sequence_start: u64,
    pub occurred_at_ms: u64,
    pub actor: AuditActor,
    pub workspace_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostgresTokenRefreshSweepReport {
    pub candidate_count: usize,
    pub attempted_count: usize,
    pub has_more: bool,
    pub reports: Vec<PostgresTokenRefreshOrchestratorReport>,
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

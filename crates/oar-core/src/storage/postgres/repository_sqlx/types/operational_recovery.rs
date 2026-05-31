use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostgresOperationalRecoveryReport {
    pub tenant_id: String,
    pub failed_audit_outbox: Vec<FailedAuditOutboxRecoveryItem>,
    pub parked_token_grants: Vec<ParkedTokenGrantRecoveryItem>,
}

impl PostgresOperationalRecoveryReport {
    pub fn has_recovery_items(&self) -> bool {
        !self.failed_audit_outbox.is_empty() || !self.parked_token_grants.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailedAuditOutboxRecoveryItem {
    pub id: i64,
    pub tenant_id: String,
    pub stream: String,
    pub aggregate_id: String,
    pub attempt_count: i32,
    pub created_at_ms: u64,
    pub payload: Option<SafeAuditOutboxPayload>,
    pub payload_safe: bool,
    pub recommended_action: OperationalRecoveryAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParkedTokenGrantRecoveryItem {
    pub grant_id: String,
    pub tenant_id: String,
    pub identity_id: String,
    pub actor_kind: ActorKind,
    pub scope_boundary: ScopeBoundary,
    pub state: TokenGrantState,
    pub safe_error: Option<String>,
    pub refreshed_at_ms: Option<u64>,
    pub reauth_required_at_ms: Option<u64>,
    pub updated_at_ms: u64,
    pub recommended_action: OperationalRecoveryAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationalRecoveryAction {
    InspectFailedAuditOutbox,
    FixFeishuRefreshConfigThenResume,
    AskUserToReauthorize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostgresOperationalRecoveryExecutionRequest {
    pub action: ConfirmedAction,
    pub confirmed_at_ms: u64,
    pub operation_id: String,
    pub occurred_at_ms: u64,
    pub outbox_next_attempt_at_ms: u64,
    pub audit_trace_id: String,
    pub kind: OperationalRecoveryExecutionKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationalRecoveryExecutionKind {
    ResumePausedAuthRefresh {
        grant_id: String,
        expected_updated_at_ms: u64,
    },
}

impl OperationalRecoveryExecutionKind {
    pub fn action_type(&self) -> &'static str {
        match self {
            Self::ResumePausedAuthRefresh { .. } => {
                "operational_recovery.resume_paused_auth_refresh"
            }
        }
    }

    pub fn target_reference_ids(&self) -> Vec<String> {
        match self {
            Self::ResumePausedAuthRefresh { grant_id, .. } => vec![grant_id.clone()],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostgresOperationalRecoveryExecutionReport {
    pub operation: OperationRecord,
    pub duplicate: bool,
    pub resumed_token_grant_id: Option<String>,
    pub events: Vec<AuditEvent>,
}

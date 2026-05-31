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

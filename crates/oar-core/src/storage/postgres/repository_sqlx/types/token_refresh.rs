use super::*;

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

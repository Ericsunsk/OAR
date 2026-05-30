use super::*;

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

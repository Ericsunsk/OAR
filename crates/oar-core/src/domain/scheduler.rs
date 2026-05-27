#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerJobKind {
    TokenRefreshSweep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerJobStatus {
    Pending,
    Running,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchedulerJobLease {
    pub id: String,
    pub tenant_id: String,
    pub job_kind: SchedulerJobKind,
    pub status: SchedulerJobStatus,
    pub attempt_count: u32,
    pub lease_id: String,
    pub lease_until_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchedulerLeaseAcquire {
    Acquired(SchedulerJobLease),
    Busy { retry_after_ms: u64 },
    NotDue { next_due_ms: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerJobOutcome {
    Succeeded,
    Noop,
    FailedSafe,
    LeaseLost,
    SkippedBusy,
    SkippedNotDue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchedulerJobAttemptReport {
    pub tenant_id: String,
    pub job_kind: SchedulerJobKind,
    pub lease_id: Option<String>,
    pub started_at_ms: u64,
    pub finished_at_ms: u64,
    pub outcome: SchedulerJobOutcome,
    pub safe_error_code: Option<String>,
}

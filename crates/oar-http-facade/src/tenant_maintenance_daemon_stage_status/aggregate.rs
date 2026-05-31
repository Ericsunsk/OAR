use crate::tenant_maintenance_daemon_failure::TenantMaintenanceDaemonFailureCode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TenantMaintenanceStageAggregateInner {
    pub(super) successful_runs: usize,
    pub(super) degraded_runs: usize,
    pub(super) failed_runs: usize,
    pub(super) last_status: Option<TenantMaintenanceStageHealth>,
    pub(super) last_failure_code: Option<TenantMaintenanceDaemonFailureCode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TenantMaintenanceStageHealth {
    Succeeded,
    Degraded,
    Failed,
}

impl TenantMaintenanceStageHealth {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Degraded => "degraded",
            Self::Failed => "failed",
        }
    }
}

impl TenantMaintenanceStageAggregateInner {
    pub(super) fn empty() -> Self {
        Self {
            successful_runs: 0,
            degraded_runs: 0,
            failed_runs: 0,
            last_status: None,
            last_failure_code: None,
        }
    }

    pub(super) fn record(
        &mut self,
        health: TenantMaintenanceStageHealth,
        failure_code: Option<TenantMaintenanceDaemonFailureCode>,
    ) {
        match health {
            TenantMaintenanceStageHealth::Succeeded => {
                self.successful_runs = self.successful_runs.saturating_add(1)
            }
            TenantMaintenanceStageHealth::Degraded => {
                self.degraded_runs = self.degraded_runs.saturating_add(1)
            }
            TenantMaintenanceStageHealth::Failed => {
                self.failed_runs = self.failed_runs.saturating_add(1)
            }
        }
        self.last_status = Some(health);
        self.last_failure_code = failure_code;
    }
}

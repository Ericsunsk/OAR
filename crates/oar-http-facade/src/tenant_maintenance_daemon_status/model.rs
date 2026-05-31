use oar_core::storage::postgres::PostgresTenantMaintenanceReport;
use oar_runtime::{
    DiscoveringRuntimeRoundReport, DiscoveringRuntimeRunReport, RuntimeRegistryRunReport,
    RuntimeTickReport,
};

use crate::tenant_maintenance_daemon_failure::{
    classify_failure_code, TenantMaintenanceDaemonFailureCode,
};
use crate::tenant_maintenance_daemon_stage_status::{
    TenantMaintenanceDaemonStagesInner, TenantMaintenanceDaemonStagesSnapshot,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TenantMaintenanceDaemonState {
    Disabled,
    Configured,
    Running,
    Stopped,
    Failed,
}

impl TenantMaintenanceDaemonState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Configured => "configured",
            Self::Running => "running",
            Self::Stopped => "stopped",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TenantMaintenanceRoundStatus {
    Succeeded,
    Degraded,
    Failed,
}

impl TenantMaintenanceRoundStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Degraded => "degraded",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TenantMaintenanceDaemonStatusSnapshot {
    pub(crate) enabled: bool,
    pub(crate) state: &'static str,
    pub(crate) successful_rounds: usize,
    pub(crate) failed_rounds: usize,
    pub(crate) failed_tenant_ticks: usize,
    pub(crate) daemon_failures: usize,
    pub(crate) last_round_status: Option<&'static str>,
    pub(crate) last_round_tenant_count: usize,
    pub(crate) last_round_failed_tenant_count: usize,
    pub(crate) last_failure_code: Option<&'static str>,
    pub(crate) last_daemon_failure_code: Option<&'static str>,
    pub(crate) stages: TenantMaintenanceDaemonStagesSnapshot,
}

impl TenantMaintenanceDaemonStatusSnapshot {
    pub(crate) fn disabled() -> Self {
        Self {
            enabled: false,
            state: TenantMaintenanceDaemonState::Disabled.as_str(),
            successful_rounds: 0,
            failed_rounds: 0,
            failed_tenant_ticks: 0,
            daemon_failures: 0,
            last_round_status: None,
            last_round_tenant_count: 0,
            last_round_failed_tenant_count: 0,
            last_failure_code: None,
            last_daemon_failure_code: None,
            stages: TenantMaintenanceDaemonStagesSnapshot::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TenantMaintenanceDaemonStatusInner {
    enabled: bool,
    state: TenantMaintenanceDaemonState,
    successful_rounds: usize,
    failed_rounds: usize,
    failed_tenant_ticks: usize,
    daemon_failures: usize,
    last_round_status: Option<TenantMaintenanceRoundStatus>,
    last_round_tenant_count: usize,
    last_round_failed_tenant_count: usize,
    last_failure_code: Option<TenantMaintenanceDaemonFailureCode>,
    last_daemon_failure_code: Option<TenantMaintenanceDaemonFailureCode>,
    stages: TenantMaintenanceDaemonStagesInner,
}

impl TenantMaintenanceDaemonStatusInner {
    pub(super) fn new(enabled: bool) -> Self {
        let state = if enabled {
            TenantMaintenanceDaemonState::Configured
        } else {
            TenantMaintenanceDaemonState::Disabled
        };
        Self {
            enabled,
            state,
            successful_rounds: 0,
            failed_rounds: 0,
            failed_tenant_ticks: 0,
            daemon_failures: 0,
            last_round_status: None,
            last_round_tenant_count: 0,
            last_round_failed_tenant_count: 0,
            last_failure_code: None,
            last_daemon_failure_code: None,
            stages: TenantMaintenanceDaemonStagesInner::empty(),
        }
    }

    pub(super) fn snapshot(&self) -> TenantMaintenanceDaemonStatusSnapshot {
        TenantMaintenanceDaemonStatusSnapshot {
            enabled: self.enabled,
            state: self.state.as_str(),
            successful_rounds: self.successful_rounds,
            failed_rounds: self.failed_rounds,
            failed_tenant_ticks: self.failed_tenant_ticks,
            daemon_failures: self.daemon_failures,
            last_round_status: self
                .last_round_status
                .map(TenantMaintenanceRoundStatus::as_str),
            last_round_tenant_count: self.last_round_tenant_count,
            last_round_failed_tenant_count: self.last_round_failed_tenant_count,
            last_failure_code: self
                .last_failure_code
                .map(TenantMaintenanceDaemonFailureCode::as_str),
            last_daemon_failure_code: self
                .last_daemon_failure_code
                .map(TenantMaintenanceDaemonFailureCode::as_str),
            stages: self.stages.snapshot(),
        }
    }

    pub(super) fn mark_running(&mut self) {
        self.state = TenantMaintenanceDaemonState::Running;
        self.last_failure_code = None;
        self.last_daemon_failure_code = None;
    }

    pub(super) fn record_round(
        &mut self,
        round: &DiscoveringRuntimeRoundReport<PostgresTenantMaintenanceReport>,
    ) {
        self.state = TenantMaintenanceDaemonState::Running;
        match round {
            DiscoveringRuntimeRoundReport::Succeeded(report) => {
                self.successful_rounds = self.successful_rounds.saturating_add(1);
                self.failed_tenant_ticks = self
                    .failed_tenant_ticks
                    .saturating_add(registry_report_failed_ticks(report));
                set_registry_round_status(self, report);
            }
            DiscoveringRuntimeRoundReport::Failed(failure) => {
                self.failed_rounds = self.failed_rounds.saturating_add(1);
                apply_failed_round(self, &failure.safe_error);
            }
        }
    }

    pub(super) fn record_tenant_report(&mut self, report: &PostgresTenantMaintenanceReport) {
        self.stages.record_report(report);
    }

    pub(super) fn mark_stopped(
        &mut self,
        report: &DiscoveringRuntimeRunReport<PostgresTenantMaintenanceReport>,
    ) {
        self.state = TenantMaintenanceDaemonState::Stopped;
        self.successful_rounds = report.successful_rounds;
        self.failed_rounds = report.failed_rounds;
        if let Some(round) = &report.last_round {
            match round {
                DiscoveringRuntimeRoundReport::Succeeded(report) => {
                    set_registry_round_status(self, report);
                }
                DiscoveringRuntimeRoundReport::Failed(failure) => {
                    apply_failed_round(self, &failure.safe_error);
                }
            }
        }
    }

    pub(super) fn mark_daemon_failed(&mut self, code: TenantMaintenanceDaemonFailureCode) {
        self.state = TenantMaintenanceDaemonState::Failed;
        self.daemon_failures = self.daemon_failures.saturating_add(1);
        self.last_daemon_failure_code = Some(code);
    }
}

fn registry_report_failed_ticks(
    report: &RuntimeRegistryRunReport<PostgresTenantMaintenanceReport>,
) -> usize {
    report
        .tenant_reports
        .iter()
        .map(|tenant| tenant.failed_ticks)
        .sum()
}

fn set_registry_round_status(
    inner: &mut TenantMaintenanceDaemonStatusInner,
    report: &RuntimeRegistryRunReport<PostgresTenantMaintenanceReport>,
) {
    let failed_tenant_count = registry_report_last_failed_tenant_count(report);
    inner.last_round_tenant_count = report.tenant_reports.len();
    inner.last_round_failed_tenant_count = failed_tenant_count;
    if failed_tenant_count == 0 {
        inner.last_round_status = Some(TenantMaintenanceRoundStatus::Succeeded);
        inner.last_failure_code = None;
    } else {
        inner.last_round_status = Some(TenantMaintenanceRoundStatus::Degraded);
        inner.last_failure_code = registry_report_first_last_tick_failure_code(report);
    }
}

fn apply_failed_round(inner: &mut TenantMaintenanceDaemonStatusInner, safe_error: impl AsRef<str>) {
    inner.last_round_tenant_count = 0;
    inner.last_round_failed_tenant_count = 0;
    inner.last_round_status = Some(TenantMaintenanceRoundStatus::Failed);
    inner.last_failure_code = Some(classify_failure_code(safe_error));
}

fn registry_report_last_failed_tenant_count(
    report: &RuntimeRegistryRunReport<PostgresTenantMaintenanceReport>,
) -> usize {
    report
        .tenant_reports
        .iter()
        .filter(|tenant| matches!(tenant.last_tick, Some(RuntimeTickReport::Failed(_))))
        .count()
}

fn registry_report_first_last_tick_failure_code(
    report: &RuntimeRegistryRunReport<PostgresTenantMaintenanceReport>,
) -> Option<TenantMaintenanceDaemonFailureCode> {
    report.tenant_reports.iter().find_map(|tenant| {
        if let Some(RuntimeTickReport::Failed(failure)) = &tenant.last_tick {
            Some(classify_failure_code(&failure.safe_error))
        } else {
            None
        }
    })
}

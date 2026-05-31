use std::fmt;
use std::sync::{Arc, RwLock};

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
pub(crate) enum TenantMaintenanceDaemonState {
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
struct TenantMaintenanceDaemonStatusInner {
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

#[derive(Clone)]
pub(crate) struct TenantMaintenanceDaemonStatusHandle {
    inner: Arc<RwLock<TenantMaintenanceDaemonStatusInner>>,
}

impl Default for TenantMaintenanceDaemonStatusHandle {
    fn default() -> Self {
        Self::for_enabled(false)
    }
}

impl fmt::Debug for TenantMaintenanceDaemonStatusHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TenantMaintenanceDaemonStatusHandle")
            .field("snapshot", &self.snapshot())
            .finish()
    }
}

impl TenantMaintenanceDaemonStatusHandle {
    pub(crate) fn for_enabled(enabled: bool) -> Self {
        let state = if enabled {
            TenantMaintenanceDaemonState::Configured
        } else {
            TenantMaintenanceDaemonState::Disabled
        };
        Self {
            inner: Arc::new(RwLock::new(TenantMaintenanceDaemonStatusInner {
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
            })),
        }
    }

    pub(crate) fn snapshot(&self) -> TenantMaintenanceDaemonStatusSnapshot {
        let inner = self.read_inner();
        TenantMaintenanceDaemonStatusSnapshot {
            enabled: inner.enabled,
            state: inner.state.as_str(),
            successful_rounds: inner.successful_rounds,
            failed_rounds: inner.failed_rounds,
            failed_tenant_ticks: inner.failed_tenant_ticks,
            daemon_failures: inner.daemon_failures,
            last_round_status: inner
                .last_round_status
                .map(TenantMaintenanceRoundStatus::as_str),
            last_round_tenant_count: inner.last_round_tenant_count,
            last_round_failed_tenant_count: inner.last_round_failed_tenant_count,
            last_failure_code: inner
                .last_failure_code
                .map(TenantMaintenanceDaemonFailureCode::as_str),
            last_daemon_failure_code: inner
                .last_daemon_failure_code
                .map(TenantMaintenanceDaemonFailureCode::as_str),
            stages: inner.stages.snapshot(),
        }
    }

    pub(crate) fn mark_running(&self) {
        self.update_inner(|inner| {
            inner.state = TenantMaintenanceDaemonState::Running;
            inner.last_failure_code = None;
            inner.last_daemon_failure_code = None;
        });
    }

    pub(crate) fn record_round(
        &self,
        round: &DiscoveringRuntimeRoundReport<PostgresTenantMaintenanceReport>,
    ) {
        self.update_inner(|inner| {
            inner.state = TenantMaintenanceDaemonState::Running;
            match round {
                DiscoveringRuntimeRoundReport::Succeeded(report) => {
                    inner.successful_rounds = inner.successful_rounds.saturating_add(1);
                    inner.failed_tenant_ticks = inner
                        .failed_tenant_ticks
                        .saturating_add(registry_report_failed_ticks(report));
                    set_registry_round_status(inner, report);
                }
                DiscoveringRuntimeRoundReport::Failed(failure) => {
                    inner.failed_rounds = inner.failed_rounds.saturating_add(1);
                    apply_failed_round(inner, &failure.safe_error);
                }
            }
        });
    }

    pub(crate) fn record_tenant_report(&self, report: &PostgresTenantMaintenanceReport) {
        self.update_inner(|inner| {
            inner.stages.record_report(report);
        });
    }

    pub(crate) fn mark_stopped(
        &self,
        report: &DiscoveringRuntimeRunReport<PostgresTenantMaintenanceReport>,
    ) {
        self.update_inner(|inner| {
            inner.state = TenantMaintenanceDaemonState::Stopped;
            inner.successful_rounds = report.successful_rounds;
            inner.failed_rounds = report.failed_rounds;
            if let Some(round) = &report.last_round {
                match round {
                    DiscoveringRuntimeRoundReport::Succeeded(report) => {
                        set_registry_round_status(inner, report);
                    }
                    DiscoveringRuntimeRoundReport::Failed(failure) => {
                        apply_failed_round(inner, &failure.safe_error);
                    }
                }
            }
        });
    }

    pub(crate) fn mark_daemon_failed(&self, code: TenantMaintenanceDaemonFailureCode) {
        self.update_inner(|inner| {
            inner.state = TenantMaintenanceDaemonState::Failed;
            inner.daemon_failures = inner.daemon_failures.saturating_add(1);
            inner.last_daemon_failure_code = Some(code);
        });
    }

    fn read_inner(&self) -> TenantMaintenanceDaemonStatusInner {
        self.inner
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn update_inner(&self, update: impl FnOnce(&mut TenantMaintenanceDaemonStatusInner)) {
        let mut inner = self
            .inner
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        update(&mut inner);
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

#[cfg(test)]
mod tests {
    use oar_runtime::{
        RuntimeRegistryRunReport, RuntimeTenantReport, RuntimeTickFailure, RuntimeTickReport,
    };

    use super::*;

    #[test]
    fn daemon_status_records_degraded_round_without_tenant_or_secret_detail() {
        let status = TenantMaintenanceDaemonStatusHandle::for_enabled(true);
        let report = RuntimeRegistryRunReport {
            tenant_reports: vec![RuntimeTenantReport {
                tenant_id: "tenant_secret_id".to_string(),
                successful_ticks: 0,
                failed_ticks: 1,
                last_tick: Some(RuntimeTickReport::Failed(RuntimeTickFailure {
                    safe_error: "tenant_maintenance_runtime_tick_failed: connection".to_string(),
                })),
            }],
            completed_rounds: 1,
            cancelled: false,
        };

        status.mark_running();
        status.record_round(&DiscoveringRuntimeRoundReport::Succeeded(report));
        let snapshot = status.snapshot();
        let rendered = format!("{status:?} {snapshot:?}");

        assert!(snapshot.enabled);
        assert_eq!(snapshot.state, "running");
        assert_eq!(snapshot.successful_rounds, 1);
        assert_eq!(snapshot.failed_rounds, 0);
        assert_eq!(snapshot.failed_tenant_ticks, 1);
        assert_eq!(snapshot.last_round_status, Some("degraded"));
        assert_eq!(snapshot.last_round_tenant_count, 1);
        assert_eq!(snapshot.last_round_failed_tenant_count, 1);
        assert_eq!(
            snapshot.last_failure_code,
            Some("tenant_maintenance_runtime_tick_failed")
        );
        assert!(!rendered.contains("tenant_secret_id"));
        assert!(!rendered.contains("webhook-secret"));
    }

    #[test]
    fn daemon_status_records_failed_round_as_safe_summary() {
        let status = TenantMaintenanceDaemonStatusHandle::for_enabled(true);

        status.record_round(&DiscoveringRuntimeRoundReport::Failed(RuntimeTickFailure {
            safe_error: "tenant_maintenance_discovery_invalid: empty_registry".to_string(),
        }));
        let snapshot = status.snapshot();

        assert_eq!(snapshot.state, "running");
        assert_eq!(snapshot.successful_rounds, 0);
        assert_eq!(snapshot.failed_rounds, 1);
        assert_eq!(snapshot.failed_tenant_ticks, 0);
        assert_eq!(snapshot.last_round_status, Some("failed"));
        assert_eq!(snapshot.last_round_tenant_count, 0);
        assert_eq!(snapshot.last_round_failed_tenant_count, 0);
        assert_eq!(
            snapshot.last_failure_code,
            Some("tenant_maintenance_discovery_invalid_empty_registry")
        );
    }

    #[test]
    fn daemon_status_redacts_suspicious_failure_code() {
        let status = TenantMaintenanceDaemonStatusHandle::for_enabled(true);

        status.mark_daemon_failed(classify_failure_code(
            "refresh_token webhook-secret authorization fingerprint",
        ));
        let snapshot = status.snapshot();

        assert_eq!(
            snapshot.last_daemon_failure_code,
            Some("tenant_maintenance_failure")
        );
        assert_eq!(snapshot.failed_rounds, 0);
        assert_eq!(snapshot.daemon_failures, 1);
        assert!(!format!("{snapshot:?}").contains("refresh_token"));
        assert!(!format!("{snapshot:?}").contains("webhook-secret"));
    }
}

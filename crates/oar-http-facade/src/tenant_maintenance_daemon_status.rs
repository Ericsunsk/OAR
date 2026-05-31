use std::fmt;
use std::sync::{Arc, RwLock};

use oar_core::storage::postgres::PostgresTenantMaintenanceReport;
use oar_runtime::{
    DiscoveringRuntimeRoundReport, DiscoveringRuntimeRunReport, RuntimeRegistryRunReport,
    RuntimeTickReport,
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
    pub(crate) last_round_status: Option<&'static str>,
    pub(crate) last_round_tenant_count: usize,
    pub(crate) last_round_failed_tenant_count: usize,
    pub(crate) last_failure_code: Option<&'static str>,
}

impl TenantMaintenanceDaemonStatusSnapshot {
    pub(crate) fn disabled() -> Self {
        Self {
            enabled: false,
            state: TenantMaintenanceDaemonState::Disabled.as_str(),
            successful_rounds: 0,
            failed_rounds: 0,
            failed_tenant_ticks: 0,
            last_round_status: None,
            last_round_tenant_count: 0,
            last_round_failed_tenant_count: 0,
            last_failure_code: None,
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
    last_round_status: Option<TenantMaintenanceRoundStatus>,
    last_round_tenant_count: usize,
    last_round_failed_tenant_count: usize,
    last_failure_code: Option<&'static str>,
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
                last_round_status: None,
                last_round_tenant_count: 0,
                last_round_failed_tenant_count: 0,
                last_failure_code: None,
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
            last_round_status: inner
                .last_round_status
                .map(TenantMaintenanceRoundStatus::as_str),
            last_round_tenant_count: inner.last_round_tenant_count,
            last_round_failed_tenant_count: inner.last_round_failed_tenant_count,
            last_failure_code: inner.last_failure_code,
        }
    }

    pub(crate) fn mark_running(&self) {
        self.update_inner(|inner| {
            inner.state = TenantMaintenanceDaemonState::Running;
            inner.last_failure_code = None;
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

    pub(crate) fn mark_failed(&self, safe_error: impl AsRef<str>) {
        self.update_inner(|inner| {
            inner.state = TenantMaintenanceDaemonState::Failed;
            inner.failed_rounds = inner.failed_rounds.saturating_add(1);
            apply_failed_round(inner, safe_error);
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
) -> Option<&'static str> {
    report.tenant_reports.iter().find_map(|tenant| {
        if let Some(RuntimeTickReport::Failed(failure)) = &tenant.last_tick {
            Some(classify_failure_code(&failure.safe_error))
        } else {
            None
        }
    })
}

fn classify_failure_code(value: impl AsRef<str>) -> &'static str {
    let value = value.as_ref().trim();
    let public_code = match value {
        "tenant_maintenance_daemon_stopped_unexpectedly" => {
            Some("tenant_maintenance_daemon_stopped_unexpectedly")
        }
        "tenant_maintenance_daemon_task_failed" => Some("tenant_maintenance_daemon_task_failed"),
        "tenant_maintenance_daemon_missing_persistence" => {
            Some("tenant_maintenance_daemon_missing_persistence")
        }
        "tenant_maintenance_daemon_missing_feishu_auth" => {
            Some("tenant_maintenance_daemon_missing_feishu_auth")
        }
        "tenant_maintenance_daemon_runtime_config_invalid" => {
            Some("tenant_maintenance_daemon_runtime_config_invalid")
        }
        "tenant_maintenance_daemon_worker_config_invalid" => {
            Some("tenant_maintenance_daemon_worker_config_invalid")
        }
        "tenant_maintenance_daemon_refresh_adapter_build_failed" => {
            Some("tenant_maintenance_daemon_refresh_adapter_build_failed")
        }
        "tenant_maintenance_daemon_webhook_sink_build_failed" => {
            Some("tenant_maintenance_daemon_webhook_sink_build_failed")
        }
        "tenant_maintenance_discovery_invalid: empty_registry" => {
            Some("tenant_maintenance_discovery_invalid_empty_registry")
        }
        "tenant_maintenance_discovery_invalid: empty_tenant_id" => {
            Some("tenant_maintenance_discovery_invalid_empty_tenant_id")
        }
        "tenant_maintenance_discovery_invalid: duplicate_tenant_id" => {
            Some("tenant_maintenance_discovery_invalid_duplicate_tenant_id")
        }
        "tenant_maintenance_registry_invalid: empty_registry" => {
            Some("tenant_maintenance_registry_invalid_empty_registry")
        }
        "tenant_maintenance_registry_invalid: empty_tenant_id" => {
            Some("tenant_maintenance_registry_invalid_empty_tenant_id")
        }
        "tenant_maintenance_registry_invalid: duplicate_tenant_id" => {
            Some("tenant_maintenance_registry_invalid_duplicate_tenant_id")
        }
        _ => None,
    };
    if let Some(code) = public_code {
        return code;
    }

    if value.starts_with("tenant_maintenance_runtime_tick_failed:") {
        return "tenant_maintenance_runtime_tick_failed";
    }
    if value.starts_with("tenant_maintenance_runtime_stage_failed:") {
        return "tenant_maintenance_runtime_stage_failed";
    }
    if value.starts_with("tenant_maintenance_registry_build_failed:") {
        return "tenant_maintenance_registry_build_failed";
    }
    "tenant_maintenance_failure"
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

        status.mark_failed("refresh_token webhook-secret authorization fingerprint");
        let snapshot = status.snapshot();

        assert_eq!(
            snapshot.last_failure_code,
            Some("tenant_maintenance_failure")
        );
        assert!(!format!("{snapshot:?}").contains("refresh_token"));
        assert!(!format!("{snapshot:?}").contains("webhook-secret"));
    }
}

use std::fmt;
use std::sync::{Arc, RwLock};

use oar_core::storage::postgres::PostgresTenantMaintenanceReport;
use oar_runtime::{DiscoveringRuntimeRoundReport, DiscoveringRuntimeRunReport};

use crate::tenant_maintenance_daemon_failure::TenantMaintenanceDaemonFailureCode;

mod model;

pub(crate) use model::TenantMaintenanceDaemonStatusSnapshot;

use model::TenantMaintenanceDaemonStatusInner;

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
        Self {
            inner: Arc::new(RwLock::new(TenantMaintenanceDaemonStatusInner::new(
                enabled,
            ))),
        }
    }

    pub(crate) fn snapshot(&self) -> TenantMaintenanceDaemonStatusSnapshot {
        self.read_inner().snapshot()
    }

    pub(crate) fn mark_running(&self) {
        self.update_inner(TenantMaintenanceDaemonStatusInner::mark_running);
    }

    pub(crate) fn record_round(
        &self,
        round: &DiscoveringRuntimeRoundReport<PostgresTenantMaintenanceReport>,
    ) {
        self.update_inner(|inner| inner.record_round(round));
    }

    pub(crate) fn record_tenant_report(&self, report: &PostgresTenantMaintenanceReport) {
        self.update_inner(|inner| inner.record_tenant_report(report));
    }

    pub(crate) fn mark_stopped(
        &self,
        report: &DiscoveringRuntimeRunReport<PostgresTenantMaintenanceReport>,
    ) {
        self.update_inner(|inner| inner.mark_stopped(report));
    }

    pub(crate) fn mark_daemon_failed(&self, code: TenantMaintenanceDaemonFailureCode) {
        self.update_inner(|inner| inner.mark_daemon_failed(code));
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

#[cfg(test)]
mod tests {
    use oar_runtime::{
        DiscoveringRuntimeRunReport, RuntimeRegistryRunReport, RuntimeTenantReport,
        RuntimeTickFailure, RuntimeTickReport,
    };

    use super::*;
    use crate::tenant_maintenance_daemon_failure::classify_failure_code;

    #[test]
    fn daemon_status_initializes_enabled_and_disabled_snapshots() {
        let enabled = TenantMaintenanceDaemonStatusHandle::for_enabled(true).snapshot();
        let disabled = TenantMaintenanceDaemonStatusHandle::for_enabled(false).snapshot();

        assert!(enabled.enabled);
        assert_eq!(enabled.state, "configured");
        assert_eq!(enabled.successful_rounds, 0);
        assert_eq!(enabled.failed_rounds, 0);
        assert_eq!(enabled.last_round_status, None);
        assert!(!disabled.enabled);
        assert_eq!(disabled.state, "disabled");
    }

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
    fn daemon_status_records_succeeded_round() {
        let status = TenantMaintenanceDaemonStatusHandle::for_enabled(true);
        let report = RuntimeRegistryRunReport {
            tenant_reports: vec![
                tenant_report("tenant_a", 0, None),
                tenant_report("tenant_b", 0, None),
            ],
            completed_rounds: 1,
            cancelled: false,
        };

        status.record_round(&DiscoveringRuntimeRoundReport::Succeeded(report));
        let snapshot = status.snapshot();

        assert_eq!(snapshot.state, "running");
        assert_eq!(snapshot.successful_rounds, 1);
        assert_eq!(snapshot.failed_rounds, 0);
        assert_eq!(snapshot.failed_tenant_ticks, 0);
        assert_eq!(snapshot.last_round_status, Some("succeeded"));
        assert_eq!(snapshot.last_round_tenant_count, 2);
        assert_eq!(snapshot.last_round_failed_tenant_count, 0);
        assert_eq!(snapshot.last_failure_code, None);
    }

    #[test]
    fn daemon_status_mark_stopped_preserves_last_round_summary() {
        let status = TenantMaintenanceDaemonStatusHandle::for_enabled(true);
        status.record_round(&DiscoveringRuntimeRoundReport::Failed(RuntimeTickFailure {
            safe_error: "tenant_maintenance_discovery_failed".to_string(),
        }));
        let report = DiscoveringRuntimeRunReport {
            successful_rounds: 7,
            failed_rounds: 2,
            last_round: Some(DiscoveringRuntimeRoundReport::Succeeded(
                RuntimeRegistryRunReport {
                    tenant_reports: vec![
                        tenant_report("tenant_a", 0, None),
                        tenant_report(
                            "tenant_b",
                            3,
                            Some("tenant_maintenance_runtime_tick_failed: timeout"),
                        ),
                    ],
                    completed_rounds: 9,
                    cancelled: true,
                },
            )),
            cancelled: true,
        };

        status.mark_stopped(&report);
        let snapshot = status.snapshot();

        assert_eq!(snapshot.state, "stopped");
        assert_eq!(snapshot.successful_rounds, 7);
        assert_eq!(snapshot.failed_rounds, 2);
        assert_eq!(snapshot.last_round_status, Some("degraded"));
        assert_eq!(snapshot.last_round_tenant_count, 2);
        assert_eq!(snapshot.last_round_failed_tenant_count, 1);
        assert_eq!(
            snapshot.last_failure_code,
            Some("tenant_maintenance_runtime_tick_failed")
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

    fn tenant_report(
        tenant_id: &str,
        failed_ticks: usize,
        last_failure: Option<&str>,
    ) -> RuntimeTenantReport<PostgresTenantMaintenanceReport> {
        RuntimeTenantReport {
            tenant_id: tenant_id.to_string(),
            successful_ticks: usize::from(failed_ticks == 0),
            failed_ticks,
            last_tick: last_failure.map(|safe_error| {
                RuntimeTickReport::Failed(RuntimeTickFailure {
                    safe_error: safe_error.to_string(),
                })
            }),
        }
    }
}

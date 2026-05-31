use oar_core::storage::postgres::PostgresTenantMaintenanceReport;

mod aggregate;
mod outbox_drain;
mod scheduled_sweep;

pub(crate) use outbox_drain::TenantMaintenanceOutboxDrainStageSnapshot;
pub(crate) use scheduled_sweep::TenantMaintenanceScheduledSweepStageSnapshot;

use outbox_drain::{record_outbox_drain_status, TenantMaintenanceOutboxDrainStageInner};
use scheduled_sweep::{record_scheduled_sweep_status, TenantMaintenanceScheduledSweepStageInner};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TenantMaintenanceDaemonStagesSnapshot {
    pub(crate) scheduled_sweep: TenantMaintenanceScheduledSweepStageSnapshot,
    pub(crate) outbox_drain: TenantMaintenanceOutboxDrainStageSnapshot,
}

impl TenantMaintenanceDaemonStagesSnapshot {
    pub(crate) fn empty() -> Self {
        Self {
            scheduled_sweep: TenantMaintenanceScheduledSweepStageSnapshot::empty(),
            outbox_drain: TenantMaintenanceOutboxDrainStageSnapshot::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TenantMaintenanceDaemonStagesInner {
    scheduled_sweep: TenantMaintenanceScheduledSweepStageInner,
    outbox_drain: TenantMaintenanceOutboxDrainStageInner,
}

impl TenantMaintenanceDaemonStagesInner {
    pub(crate) fn empty() -> Self {
        Self {
            scheduled_sweep: TenantMaintenanceScheduledSweepStageInner::empty(),
            outbox_drain: TenantMaintenanceOutboxDrainStageInner::empty(),
        }
    }

    pub(crate) fn snapshot(&self) -> TenantMaintenanceDaemonStagesSnapshot {
        TenantMaintenanceDaemonStagesSnapshot {
            scheduled_sweep: self.scheduled_sweep.snapshot(),
            outbox_drain: self.outbox_drain.snapshot(),
        }
    }

    pub(crate) fn record_report(&mut self, report: &PostgresTenantMaintenanceReport) {
        record_scheduled_sweep_status(&mut self.scheduled_sweep, &report.scheduled_sweep);
        record_outbox_drain_status(&mut self.outbox_drain, &report.outbox_drain);
    }
}

#[cfg(test)]
mod tests {
    use oar_core::domain::scheduler::{
        SchedulerJobAttemptReport, SchedulerJobKind, SchedulerJobOutcome, SchedulerLeaseAcquire,
    };
    use oar_core::storage::postgres::{
        audit_outbox_worker::AuditOutboxDrainReport,
        tenant_maintenance::{
            PostgresTenantMaintenanceStage, PostgresTenantMaintenanceStageFailure,
        },
        PostgresTenantMaintenanceReport, PostgresTokenRefreshSweepReport,
        TokenRefreshScheduledSweepReport,
    };

    use super::*;

    #[test]
    fn stage_status_records_metrics_without_tenant_detail() {
        let mut status = TenantMaintenanceDaemonStagesInner::empty();
        let report = PostgresTenantMaintenanceReport {
            scheduled_sweep: PostgresTenantMaintenanceStage::Succeeded(scheduled_sweep_report(
                SchedulerJobOutcome::Succeeded,
                3,
                2,
                true,
            )),
            outbox_drain: PostgresTenantMaintenanceStage::Succeeded(AuditOutboxDrainReport {
                claimed: 4,
                sent: 2,
                retryable: 1,
                failed: 0,
                exhausted: 0,
                stale: 0,
            }),
        };

        status.record_report(&report);
        let snapshot = status.snapshot();
        let rendered = format!("{snapshot:?}");

        assert_eq!(snapshot.scheduled_sweep.successful_runs, 1);
        assert_eq!(snapshot.scheduled_sweep.last_status, Some("succeeded"));
        assert_eq!(snapshot.scheduled_sweep.last_outcome, Some("succeeded"));
        assert_eq!(snapshot.scheduled_sweep.last_candidate_count, 3);
        assert_eq!(snapshot.scheduled_sweep.last_attempted_count, 2);
        assert!(snapshot.scheduled_sweep.last_has_more);
        assert_eq!(snapshot.outbox_drain.degraded_runs, 1);
        assert_eq!(snapshot.outbox_drain.last_status, Some("degraded"));
        assert_eq!(
            snapshot.outbox_drain.last_failure_code,
            Some("tenant_maintenance_outbox_retryable")
        );
        assert_eq!(snapshot.outbox_drain.last_claimed, 4);
        assert_eq!(snapshot.outbox_drain.last_sent, 2);
        assert_eq!(snapshot.outbox_drain.last_retryable, 1);
        assert!(!rendered.contains("tenant_stage"));
    }

    #[test]
    fn stage_status_records_failed_metrics_as_codes() {
        let mut status = TenantMaintenanceDaemonStagesInner::empty();
        let report = PostgresTenantMaintenanceReport {
            scheduled_sweep: PostgresTenantMaintenanceStage::Succeeded(scheduled_sweep_report(
                SchedulerJobOutcome::LeaseLost,
                0,
                0,
                false,
            )),
            outbox_drain: PostgresTenantMaintenanceStage::Failed(
                PostgresTenantMaintenanceStageFailure {
                    safe_error: "https://sink.test?token=secret".to_string(),
                },
            ),
        };

        status.record_report(&report);
        let snapshot = status.snapshot();
        let rendered = format!("{snapshot:?}");

        assert_eq!(snapshot.scheduled_sweep.failed_runs, 1);
        assert_eq!(
            snapshot.scheduled_sweep.last_failure_code,
            Some("tenant_maintenance_scheduled_sweep_lease_lost")
        );
        assert_eq!(snapshot.outbox_drain.failed_runs, 1);
        assert_eq!(
            snapshot.outbox_drain.last_failure_code,
            Some("tenant_maintenance_failure")
        );
        assert!(!rendered.contains("sink.test"));
        assert!(!rendered.contains("secret"));
    }

    fn scheduled_sweep_report(
        outcome: SchedulerJobOutcome,
        candidate_count: usize,
        attempted_count: usize,
        has_more: bool,
    ) -> TokenRefreshScheduledSweepReport {
        TokenRefreshScheduledSweepReport {
            acquire: SchedulerLeaseAcquire::NotDue { next_due_ms: 1 },
            attempt: SchedulerJobAttemptReport {
                tenant_id: "tenant_stage".to_string(),
                job_kind: SchedulerJobKind::TokenRefreshSweep,
                lease_id: None,
                started_at_ms: 1,
                finished_at_ms: 2,
                outcome,
                safe_error_code: None,
            },
            sweep: Some(PostgresTokenRefreshSweepReport {
                candidate_count,
                attempted_count,
                has_more,
                reports: Vec::new(),
            }),
        }
    }
}

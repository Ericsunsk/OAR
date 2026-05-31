use oar_core::storage::postgres::{
    audit_outbox_worker::AuditOutboxDrainReport, tenant_maintenance::PostgresTenantMaintenanceStage,
};

use super::aggregate::{TenantMaintenanceStageAggregateInner, TenantMaintenanceStageHealth};
use crate::tenant_maintenance_daemon_failure::{
    classify_failure_code, TenantMaintenanceDaemonFailureCode,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TenantMaintenanceOutboxDrainStageSnapshot {
    pub(crate) successful_runs: usize,
    pub(crate) degraded_runs: usize,
    pub(crate) failed_runs: usize,
    pub(crate) last_status: Option<&'static str>,
    pub(crate) last_claimed: usize,
    pub(crate) last_sent: usize,
    pub(crate) last_retryable: usize,
    pub(crate) last_failed: usize,
    pub(crate) last_exhausted: usize,
    pub(crate) last_stale: usize,
    pub(crate) last_failure_code: Option<&'static str>,
}

impl TenantMaintenanceOutboxDrainStageSnapshot {
    pub(super) fn empty() -> Self {
        Self {
            successful_runs: 0,
            degraded_runs: 0,
            failed_runs: 0,
            last_status: None,
            last_claimed: 0,
            last_sent: 0,
            last_retryable: 0,
            last_failed: 0,
            last_exhausted: 0,
            last_stale: 0,
            last_failure_code: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TenantMaintenanceOutboxDrainStageInner {
    aggregate: TenantMaintenanceStageAggregateInner,
    last_claimed: usize,
    last_sent: usize,
    last_retryable: usize,
    last_failed: usize,
    last_exhausted: usize,
    last_stale: usize,
}

impl TenantMaintenanceOutboxDrainStageInner {
    pub(super) fn empty() -> Self {
        Self {
            aggregate: TenantMaintenanceStageAggregateInner::empty(),
            last_claimed: 0,
            last_sent: 0,
            last_retryable: 0,
            last_failed: 0,
            last_exhausted: 0,
            last_stale: 0,
        }
    }

    pub(super) fn snapshot(&self) -> TenantMaintenanceOutboxDrainStageSnapshot {
        TenantMaintenanceOutboxDrainStageSnapshot {
            successful_runs: self.aggregate.successful_runs,
            degraded_runs: self.aggregate.degraded_runs,
            failed_runs: self.aggregate.failed_runs,
            last_status: self
                .aggregate
                .last_status
                .map(TenantMaintenanceStageHealth::as_str),
            last_claimed: self.last_claimed,
            last_sent: self.last_sent,
            last_retryable: self.last_retryable,
            last_failed: self.last_failed,
            last_exhausted: self.last_exhausted,
            last_stale: self.last_stale,
            last_failure_code: self
                .aggregate
                .last_failure_code
                .map(TenantMaintenanceDaemonFailureCode::as_str),
        }
    }
}

pub(super) fn record_outbox_drain_status(
    status: &mut TenantMaintenanceOutboxDrainStageInner,
    stage: &PostgresTenantMaintenanceStage<AuditOutboxDrainReport>,
) {
    match stage {
        PostgresTenantMaintenanceStage::Succeeded(report) => {
            let (health, failure_code) = outbox_drain_health(report);
            status.aggregate.record(health, failure_code);
            status.last_claimed = report.claimed;
            status.last_sent = report.sent;
            status.last_retryable = report.retryable;
            status.last_failed = report.failed;
            status.last_exhausted = report.exhausted;
            status.last_stale = report.stale;
        }
        PostgresTenantMaintenanceStage::Failed(failure) => {
            status.aggregate.record(
                TenantMaintenanceStageHealth::Failed,
                Some(classify_failure_code(&failure.safe_error)),
            );
            status.last_claimed = 0;
            status.last_sent = 0;
            status.last_retryable = 0;
            status.last_failed = 0;
            status.last_exhausted = 0;
            status.last_stale = 0;
        }
    }
}

fn outbox_drain_health(
    report: &AuditOutboxDrainReport,
) -> (
    TenantMaintenanceStageHealth,
    Option<TenantMaintenanceDaemonFailureCode>,
) {
    if report.failed > 0 || report.exhausted > 0 {
        return (
            TenantMaintenanceStageHealth::Failed,
            Some(if report.exhausted > 0 {
                TenantMaintenanceDaemonFailureCode::OutboxExhausted
            } else {
                TenantMaintenanceDaemonFailureCode::OutboxFailed
            }),
        );
    }
    if report.retryable > 0 || report.stale > 0 {
        return (
            TenantMaintenanceStageHealth::Degraded,
            Some(if report.stale > 0 {
                TenantMaintenanceDaemonFailureCode::OutboxStale
            } else {
                TenantMaintenanceDaemonFailureCode::OutboxRetryable
            }),
        );
    }
    (TenantMaintenanceStageHealth::Succeeded, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outbox_drain_status_maps_counts_with_expected_precedence() {
        let cases = [
            (report(0, 0, 0, 0, 0), "succeeded", None),
            (
                report(0, 0, 1, 0, 0),
                "degraded",
                Some("tenant_maintenance_outbox_retryable"),
            ),
            (
                report(0, 0, 0, 0, 1),
                "degraded",
                Some("tenant_maintenance_outbox_stale"),
            ),
            (
                report(0, 0, 0, 1, 0),
                "failed",
                Some("tenant_maintenance_outbox_failed"),
            ),
            (
                report_with_exhausted(0, 0, 0, 0, 1, 0),
                "failed",
                Some("tenant_maintenance_outbox_exhausted"),
            ),
            (
                report_with_exhausted(0, 0, 0, 1, 1, 0),
                "failed",
                Some("tenant_maintenance_outbox_exhausted"),
            ),
            (
                report(0, 0, 1, 0, 1),
                "degraded",
                Some("tenant_maintenance_outbox_stale"),
            ),
        ];

        for (report, expected_status, expected_code) in cases {
            let mut status = TenantMaintenanceOutboxDrainStageInner::empty();
            let stage = PostgresTenantMaintenanceStage::Succeeded(report);

            record_outbox_drain_status(&mut status, &stage);
            let snapshot = status.snapshot();

            assert_eq!(snapshot.last_status, Some(expected_status));
            assert_eq!(snapshot.last_failure_code, expected_code);
        }
    }

    fn report(
        claimed: usize,
        sent: usize,
        retryable: usize,
        failed: usize,
        stale: usize,
    ) -> AuditOutboxDrainReport {
        AuditOutboxDrainReport {
            claimed,
            sent,
            retryable,
            failed,
            exhausted: 0,
            stale,
        }
    }

    fn report_with_exhausted(
        claimed: usize,
        sent: usize,
        retryable: usize,
        failed: usize,
        exhausted: usize,
        stale: usize,
    ) -> AuditOutboxDrainReport {
        AuditOutboxDrainReport {
            claimed,
            sent,
            retryable,
            failed,
            exhausted,
            stale,
        }
    }
}

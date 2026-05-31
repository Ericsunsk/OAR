use oar_core::domain::scheduler::SchedulerJobOutcome;
use oar_core::storage::postgres::{
    tenant_maintenance::PostgresTenantMaintenanceStage, TokenRefreshScheduledSweepReport,
};

use super::aggregate::{TenantMaintenanceStageAggregateInner, TenantMaintenanceStageHealth};
use crate::tenant_maintenance_daemon_failure::{
    classify_failure_code, TenantMaintenanceDaemonFailureCode,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TenantMaintenanceScheduledSweepStageSnapshot {
    pub(crate) successful_runs: usize,
    pub(crate) degraded_runs: usize,
    pub(crate) failed_runs: usize,
    pub(crate) last_status: Option<&'static str>,
    pub(crate) last_outcome: Option<&'static str>,
    pub(crate) last_candidate_count: usize,
    pub(crate) last_attempted_count: usize,
    pub(crate) last_has_more: bool,
    pub(crate) last_failure_code: Option<&'static str>,
}

impl TenantMaintenanceScheduledSweepStageSnapshot {
    pub(super) fn empty() -> Self {
        Self {
            successful_runs: 0,
            degraded_runs: 0,
            failed_runs: 0,
            last_status: None,
            last_outcome: None,
            last_candidate_count: 0,
            last_attempted_count: 0,
            last_has_more: false,
            last_failure_code: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TenantMaintenanceScheduledSweepStageInner {
    aggregate: TenantMaintenanceStageAggregateInner,
    last_outcome: Option<&'static str>,
    last_candidate_count: usize,
    last_attempted_count: usize,
    last_has_more: bool,
}

impl TenantMaintenanceScheduledSweepStageInner {
    pub(super) fn empty() -> Self {
        Self {
            aggregate: TenantMaintenanceStageAggregateInner::empty(),
            last_outcome: None,
            last_candidate_count: 0,
            last_attempted_count: 0,
            last_has_more: false,
        }
    }

    pub(super) fn snapshot(&self) -> TenantMaintenanceScheduledSweepStageSnapshot {
        TenantMaintenanceScheduledSweepStageSnapshot {
            successful_runs: self.aggregate.successful_runs,
            degraded_runs: self.aggregate.degraded_runs,
            failed_runs: self.aggregate.failed_runs,
            last_status: self
                .aggregate
                .last_status
                .map(TenantMaintenanceStageHealth::as_str),
            last_outcome: self.last_outcome,
            last_candidate_count: self.last_candidate_count,
            last_attempted_count: self.last_attempted_count,
            last_has_more: self.last_has_more,
            last_failure_code: self
                .aggregate
                .last_failure_code
                .map(TenantMaintenanceDaemonFailureCode::as_str),
        }
    }
}

pub(super) fn record_scheduled_sweep_status(
    status: &mut TenantMaintenanceScheduledSweepStageInner,
    stage: &PostgresTenantMaintenanceStage<TokenRefreshScheduledSweepReport>,
) {
    match stage {
        PostgresTenantMaintenanceStage::Succeeded(report) => {
            let (health, failure_code) = scheduled_sweep_health(report);
            status.aggregate.record(health, failure_code);
            status.last_outcome = Some(scheduler_job_outcome_status(report.attempt.outcome));
            if let Some(sweep) = report.sweep.as_ref() {
                status.last_candidate_count = sweep.candidate_count;
                status.last_attempted_count = sweep.attempted_count;
                status.last_has_more = sweep.has_more;
            } else {
                status.last_candidate_count = 0;
                status.last_attempted_count = 0;
                status.last_has_more = false;
            }
        }
        PostgresTenantMaintenanceStage::Failed(failure) => {
            status.aggregate.record(
                TenantMaintenanceStageHealth::Failed,
                Some(classify_failure_code(&failure.safe_error)),
            );
            status.last_outcome = None;
            status.last_candidate_count = 0;
            status.last_attempted_count = 0;
            status.last_has_more = false;
        }
    }
}

fn scheduled_sweep_health(
    report: &TokenRefreshScheduledSweepReport,
) -> (
    TenantMaintenanceStageHealth,
    Option<TenantMaintenanceDaemonFailureCode>,
) {
    match report.attempt.outcome {
        SchedulerJobOutcome::Succeeded
        | SchedulerJobOutcome::Noop
        | SchedulerJobOutcome::SkippedNotDue => (TenantMaintenanceStageHealth::Succeeded, None),
        SchedulerJobOutcome::SkippedBusy => (
            TenantMaintenanceStageHealth::Degraded,
            Some(TenantMaintenanceDaemonFailureCode::ScheduledSweepBusy),
        ),
        SchedulerJobOutcome::FailedSafe => (
            TenantMaintenanceStageHealth::Failed,
            Some(
                report
                    .attempt
                    .safe_error_code
                    .as_deref()
                    .map(classify_failure_code)
                    .unwrap_or(TenantMaintenanceDaemonFailureCode::ScheduledSweepFailed),
            ),
        ),
        SchedulerJobOutcome::LeaseLost => (
            TenantMaintenanceStageHealth::Failed,
            Some(TenantMaintenanceDaemonFailureCode::ScheduledSweepLeaseLost),
        ),
    }
}

fn scheduler_job_outcome_status(outcome: SchedulerJobOutcome) -> &'static str {
    match outcome {
        SchedulerJobOutcome::Succeeded => "succeeded",
        SchedulerJobOutcome::Noop => "noop",
        SchedulerJobOutcome::FailedSafe => "failed_safe",
        SchedulerJobOutcome::LeaseLost => "lease_lost",
        SchedulerJobOutcome::SkippedBusy => "skipped_busy",
        SchedulerJobOutcome::SkippedNotDue => "skipped_not_due",
    }
}

#[cfg(test)]
mod tests {
    use oar_core::domain::scheduler::{
        SchedulerJobAttemptReport, SchedulerJobKind, SchedulerLeaseAcquire,
    };
    use oar_core::storage::postgres::{
        PostgresTokenRefreshSweepReport, TokenRefreshScheduledSweepReport,
    };

    use super::*;

    #[test]
    fn scheduled_sweep_status_maps_scheduler_outcomes() {
        let cases = [
            (
                SchedulerJobOutcome::Succeeded,
                None,
                "succeeded",
                "succeeded",
                None,
            ),
            (SchedulerJobOutcome::Noop, None, "succeeded", "noop", None),
            (
                SchedulerJobOutcome::SkippedNotDue,
                None,
                "succeeded",
                "skipped_not_due",
                None,
            ),
            (
                SchedulerJobOutcome::SkippedBusy,
                None,
                "degraded",
                "skipped_busy",
                Some("tenant_maintenance_scheduled_sweep_busy"),
            ),
            (
                SchedulerJobOutcome::FailedSafe,
                None,
                "failed",
                "failed_safe",
                Some("tenant_maintenance_scheduled_sweep_failed"),
            ),
            (
                SchedulerJobOutcome::LeaseLost,
                None,
                "failed",
                "lease_lost",
                Some("tenant_maintenance_scheduled_sweep_lease_lost"),
            ),
            (
                SchedulerJobOutcome::FailedSafe,
                Some("tenant_maintenance_scheduled_sweep_busy"),
                "failed",
                "failed_safe",
                Some("tenant_maintenance_scheduled_sweep_busy"),
            ),
        ];

        for (outcome, safe_error_code, expected_status, expected_outcome, expected_code) in cases {
            let mut status = TenantMaintenanceScheduledSweepStageInner::empty();
            let stage = PostgresTenantMaintenanceStage::Succeeded(scheduled_sweep_report(
                outcome,
                safe_error_code,
            ));

            record_scheduled_sweep_status(&mut status, &stage);
            let snapshot = status.snapshot();

            assert_eq!(snapshot.last_status, Some(expected_status));
            assert_eq!(snapshot.last_outcome, Some(expected_outcome));
            assert_eq!(snapshot.last_failure_code, expected_code);
        }
    }

    fn scheduled_sweep_report(
        outcome: SchedulerJobOutcome,
        safe_error_code: Option<&'static str>,
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
                safe_error_code: safe_error_code.map(str::to_string),
            },
            sweep: Some(PostgresTokenRefreshSweepReport {
                candidate_count: 0,
                attempted_count: 0,
                has_more: false,
                reports: Vec::new(),
            }),
        }
    }
}

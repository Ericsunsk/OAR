use oar_core::domain::scheduler::SchedulerJobOutcome;
use oar_core::storage::postgres::{
    audit_outbox_worker::AuditOutboxDrainReport,
    tenant_maintenance::PostgresTenantMaintenanceStage, PostgresTenantMaintenanceReport,
    TokenRefreshScheduledSweepReport,
};

use crate::tenant_maintenance_daemon_failure::{
    classify_failure_code, TenantMaintenanceDaemonFailureCode,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TenantMaintenanceDaemonStagesSnapshot {
    pub(crate) scheduled_sweep: TenantMaintenanceScheduledSweepStageSnapshot,
    pub(crate) outbox_drain: TenantMaintenanceOutboxDrainStageSnapshot,
}

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

impl TenantMaintenanceDaemonStagesSnapshot {
    pub(crate) fn empty() -> Self {
        Self {
            scheduled_sweep: TenantMaintenanceScheduledSweepStageSnapshot::empty(),
            outbox_drain: TenantMaintenanceOutboxDrainStageSnapshot::empty(),
        }
    }
}

impl TenantMaintenanceScheduledSweepStageSnapshot {
    fn empty() -> Self {
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

impl TenantMaintenanceOutboxDrainStageSnapshot {
    fn empty() -> Self {
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
pub(crate) struct TenantMaintenanceDaemonStagesInner {
    scheduled_sweep: TenantMaintenanceScheduledSweepStageInner,
    outbox_drain: TenantMaintenanceOutboxDrainStageInner,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TenantMaintenanceScheduledSweepStageInner {
    aggregate: TenantMaintenanceStageAggregateInner,
    last_outcome: Option<&'static str>,
    last_candidate_count: usize,
    last_attempted_count: usize,
    last_has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TenantMaintenanceOutboxDrainStageInner {
    aggregate: TenantMaintenanceStageAggregateInner,
    last_claimed: usize,
    last_sent: usize,
    last_retryable: usize,
    last_failed: usize,
    last_exhausted: usize,
    last_stale: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TenantMaintenanceStageAggregateInner {
    successful_runs: usize,
    degraded_runs: usize,
    failed_runs: usize,
    last_status: Option<TenantMaintenanceStageHealth>,
    last_failure_code: Option<TenantMaintenanceDaemonFailureCode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TenantMaintenanceStageHealth {
    Succeeded,
    Degraded,
    Failed,
}

impl TenantMaintenanceStageHealth {
    fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Degraded => "degraded",
            Self::Failed => "failed",
        }
    }
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

impl TenantMaintenanceScheduledSweepStageInner {
    fn empty() -> Self {
        Self {
            aggregate: TenantMaintenanceStageAggregateInner::empty(),
            last_outcome: None,
            last_candidate_count: 0,
            last_attempted_count: 0,
            last_has_more: false,
        }
    }

    fn snapshot(&self) -> TenantMaintenanceScheduledSweepStageSnapshot {
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

impl TenantMaintenanceOutboxDrainStageInner {
    fn empty() -> Self {
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

    fn snapshot(&self) -> TenantMaintenanceOutboxDrainStageSnapshot {
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

impl TenantMaintenanceStageAggregateInner {
    fn empty() -> Self {
        Self {
            successful_runs: 0,
            degraded_runs: 0,
            failed_runs: 0,
            last_status: None,
            last_failure_code: None,
        }
    }

    fn record(
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

fn record_scheduled_sweep_status(
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

fn record_outbox_drain_status(
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

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::{
    PostgresRepositoryError, PostgresSchedulerJobRepository, PostgresTokenRefreshSweep,
    PostgresTokenRefreshSweepReport, PostgresTokenRefreshSweepRequest,
};
use crate::action::audit_event::AuditActor;
use crate::domain::scheduler::{
    SchedulerJobAttemptReport, SchedulerJobKind, SchedulerJobOutcome, SchedulerLeaseAcquire,
};
use crate::domain::token_refresh::service::AsyncAuthRefreshAdapter;

const MIN_AUDIT_SEQUENCE_WINDOW: u64 = 1_000_000;
pub const TOKEN_REFRESH_SWEEP_SCHEDULER_JOB_ID: &str = "token_refresh_sweep";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenRefreshScheduledSweepConfig {
    pub tenant_id: String,
    pub lease_id: String,
    pub lease_ms: u64,
    pub retry_delay_ms: u64,
    pub next_run_delay_ms: u64,
    pub backlog_next_run_delay_ms: u64,
    pub due_before_ms: u64,
    pub limit: u32,
    pub audit_trace_id: String,
    pub audit_sequence_start: u64,
    pub actor: AuditActor,
    pub workspace_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenRefreshScheduledSweepReport {
    pub acquire: SchedulerLeaseAcquire,
    pub attempt: SchedulerJobAttemptReport,
    pub sweep: Option<PostgresTokenRefreshSweepReport>,
}

pub struct PostgresTokenRefreshScheduledSweep<A, C = fn() -> u64>
where
    A: AsyncAuthRefreshAdapter,
    C: FnMut() -> u64,
{
    leases: PostgresSchedulerJobRepository,
    sweep: PostgresTokenRefreshSweep<A>,
    clock_ms: C,
    config: TokenRefreshScheduledSweepConfig,
}

impl<A, C> PostgresTokenRefreshScheduledSweep<A, C>
where
    A: AsyncAuthRefreshAdapter,
    C: FnMut() -> u64,
{
    pub fn new(
        leases: PostgresSchedulerJobRepository,
        sweep: PostgresTokenRefreshSweep<A>,
        clock_ms: C,
        config: TokenRefreshScheduledSweepConfig,
    ) -> Self {
        Self {
            leases,
            sweep,
            clock_ms,
            config,
        }
    }

    pub fn adapter(&self) -> &A {
        self.sweep.adapter()
    }

    pub async fn run_once(
        &mut self,
    ) -> Result<TokenRefreshScheduledSweepReport, PostgresRepositoryError> {
        let started_at_ms = self.now_ms();
        self.leases
            .insert_job_if_missing(
                TOKEN_REFRESH_SWEEP_SCHEDULER_JOB_ID,
                &self.config.tenant_id,
                SchedulerJobKind::TokenRefreshSweep,
                started_at_ms,
            )
            .await?;
        let lease_until_ms = started_at_ms.saturating_add(self.config.lease_ms);
        let acquire = self
            .leases
            .try_acquire(
                &self.config.tenant_id,
                SchedulerJobKind::TokenRefreshSweep,
                started_at_ms,
                &self.config.lease_id,
                lease_until_ms,
            )
            .await?;

        let lease = match acquire.clone() {
            SchedulerLeaseAcquire::Acquired(lease) => lease,
            SchedulerLeaseAcquire::Busy { .. } => {
                let finished_at_ms = self.now_ms();
                return Ok(TokenRefreshScheduledSweepReport {
                    acquire,
                    attempt: self.attempt_report(
                        None,
                        started_at_ms,
                        finished_at_ms,
                        SchedulerJobOutcome::SkippedBusy,
                        None,
                    ),
                    sweep: None,
                });
            }
            SchedulerLeaseAcquire::NotDue { .. } => {
                let finished_at_ms = self.now_ms();
                return Ok(TokenRefreshScheduledSweepReport {
                    acquire,
                    attempt: self.attempt_report(
                        None,
                        started_at_ms,
                        finished_at_ms,
                        SchedulerJobOutcome::SkippedNotDue,
                        None,
                    ),
                    sweep: None,
                });
            }
        };

        let sweep_result = self
            .sweep
            .run_once_for_tenant(PostgresTokenRefreshSweepRequest {
                tenant_id: self.config.tenant_id.clone(),
                due_before: ms_to_system_time(self.config.due_before_ms),
                limit: self.config.limit,
                now: ms_to_system_time(started_at_ms),
                audit_trace_id: self.config.audit_trace_id.clone(),
                audit_sequence_start: self.audit_sequence_start(lease.attempt_count),
                occurred_at_ms: started_at_ms,
                actor: self.config.actor.clone(),
                workspace_id: self.config.workspace_id.clone(),
            })
            .await;

        match sweep_result {
            Ok(sweep_report) => {
                let finished_at_ms = self.now_ms();
                let delay_ms = if sweep_report.has_more {
                    self.config.backlog_next_run_delay_ms
                } else {
                    self.config.next_run_delay_ms
                };
                let next_run_at_ms = finished_at_ms.saturating_add(delay_ms);
                let finalized = self
                    .leases
                    .complete_for_lease(&lease, finished_at_ms, next_run_at_ms)
                    .await?;
                let outcome = if finalized {
                    if sweep_report.attempted_count == 0 {
                        SchedulerJobOutcome::Noop
                    } else {
                        SchedulerJobOutcome::Succeeded
                    }
                } else {
                    SchedulerJobOutcome::LeaseLost
                };
                Ok(TokenRefreshScheduledSweepReport {
                    acquire,
                    attempt: self.attempt_report(
                        Some(lease.lease_id),
                        started_at_ms,
                        finished_at_ms,
                        outcome,
                        None,
                    ),
                    sweep: Some(sweep_report),
                })
            }
            Err(_error) => {
                let finished_at_ms = self.now_ms();
                let next_run_at_ms = finished_at_ms.saturating_add(self.config.retry_delay_ms);
                let finalized = self
                    .leases
                    .fail_for_lease(
                        &lease,
                        finished_at_ms,
                        "token_refresh_sweep_failed",
                        next_run_at_ms,
                    )
                    .await?;
                let outcome = if finalized {
                    SchedulerJobOutcome::FailedSafe
                } else {
                    SchedulerJobOutcome::LeaseLost
                };
                Ok(TokenRefreshScheduledSweepReport {
                    acquire,
                    attempt: self.attempt_report(
                        Some(lease.lease_id),
                        started_at_ms,
                        finished_at_ms,
                        outcome,
                        Some("token_refresh_sweep_failed".to_string()),
                    ),
                    sweep: None,
                })
            }
        }
    }

    fn attempt_report(
        &self,
        lease_id: Option<String>,
        started_at_ms: u64,
        finished_at_ms: u64,
        outcome: SchedulerJobOutcome,
        safe_error_code: Option<String>,
    ) -> SchedulerJobAttemptReport {
        SchedulerJobAttemptReport {
            tenant_id: self.config.tenant_id.clone(),
            job_kind: SchedulerJobKind::TokenRefreshSweep,
            lease_id,
            started_at_ms,
            finished_at_ms,
            outcome,
            safe_error_code,
        }
    }

    fn now_ms(&mut self) -> u64 {
        (self.clock_ms)()
    }

    fn audit_sequence_start(&self, attempt_count: u32) -> u64 {
        scheduled_sweep_audit_sequence_start(
            self.config.audit_sequence_start,
            attempt_count,
            self.config.limit,
        )
    }
}

fn ms_to_system_time(value: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_millis(value)
}

fn scheduled_sweep_audit_sequence_start(base: u64, attempt_count: u32, limit: u32) -> u64 {
    let attempt_index = u64::from(attempt_count.saturating_sub(1));
    let sequence_window = u64::from(limit).max(MIN_AUDIT_SEQUENCE_WINDOW);
    base.saturating_add(attempt_index.saturating_mul(sequence_window))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_sequence_start_allocates_stable_attempt_windows() {
        assert_eq!(scheduled_sweep_audit_sequence_start(81, 1, 4), 81);
        assert_eq!(scheduled_sweep_audit_sequence_start(81, 2, 4), 1_000_081);
        assert_eq!(
            scheduled_sweep_audit_sequence_start(81, 3, 2_000_000),
            4_000_081
        );
    }
}

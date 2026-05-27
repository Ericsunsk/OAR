use std::error::Error;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use tokio::time::{self, MissedTickBehavior};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use oar_core::storage::postgres::{
    PostgresRepositoryError, PostgresTenantMaintenanceReport, PostgresTenantMaintenanceWorker,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantMaintenanceRuntimeConfig {
    pub tick_interval: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TenantMaintenanceRuntimeConfigValidationError {
    ZeroTickInterval,
}

impl fmt::Display for TenantMaintenanceRuntimeConfigValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroTickInterval => write!(
                f,
                "tenant_maintenance_runtime_config_invalid: zero_tick_interval"
            ),
        }
    }
}

impl Error for TenantMaintenanceRuntimeConfigValidationError {}

impl TenantMaintenanceRuntimeConfig {
    pub fn validate(&self) -> Result<(), TenantMaintenanceRuntimeConfigValidationError> {
        if self.tick_interval.is_zero() {
            return Err(TenantMaintenanceRuntimeConfigValidationError::ZeroTickInterval);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTickFailure {
    pub safe_error: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeTickReport<T> {
    Succeeded(T),
    Failed(RuntimeTickFailure),
}

pub type RuntimeTickFuture<'a, R, E> = Pin<Box<dyn Future<Output = Result<R, E>> + Send + 'a>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeRunReport<T> {
    pub successful_ticks: usize,
    pub failed_ticks: usize,
    pub last_tick: Option<RuntimeTickReport<T>>,
    pub cancelled: bool,
}

pub trait RuntimeTick {
    type Report: Send + 'static;
    type Error: Error + Send + Sync + 'static;

    fn tick(&mut self) -> RuntimeTickFuture<'_, Self::Report, Self::Error>;
    fn safe_error(error: &Self::Error) -> String;
}

pub struct TenantMaintenanceRuntime<T>
where
    T: RuntimeTick,
{
    config: TenantMaintenanceRuntimeConfig,
    tick: T,
}

impl<T> TenantMaintenanceRuntime<T>
where
    T: RuntimeTick,
{
    pub fn try_new(
        config: TenantMaintenanceRuntimeConfig,
        tick: T,
    ) -> Result<Self, TenantMaintenanceRuntimeConfigValidationError> {
        config.validate()?;
        Ok(Self { config, tick })
    }

    pub async fn run_until_cancelled(
        &mut self,
        cancellation: &CancellationToken,
    ) -> RuntimeRunReport<T::Report> {
        let mut interval = time::interval(self.config.tick_interval);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        let mut successful_ticks = 0usize;
        let mut failed_ticks = 0usize;
        let mut last_tick = None;

        loop {
            if cancellation.is_cancelled() {
                info!("tenant maintenance runtime cancelled");
                break;
            }

            tokio::select! {
                _ = cancellation.cancelled() => {
                    info!("tenant maintenance runtime cancelled");
                    break;
                }
                _ = interval.tick() => {
                    match self.tick.tick().await {
                        Ok(report) => {
                            successful_ticks = successful_ticks.saturating_add(1);
                            last_tick = Some(RuntimeTickReport::Succeeded(report));
                        }
                        Err(error) => {
                            failed_ticks = failed_ticks.saturating_add(1);
                            let safe_error = T::safe_error(&error);
                            warn!(safe_error = %safe_error, "tenant maintenance runtime tick failed");
                            last_tick = Some(RuntimeTickReport::Failed(RuntimeTickFailure { safe_error }));
                        }
                    }
                }
            }
        }

        RuntimeRunReport {
            successful_ticks,
            failed_ticks,
            last_tick,
            cancelled: true,
        }
    }
}

impl<R, D, C> RuntimeTick for PostgresTenantMaintenanceWorker<R, D, C>
where
    R: oar_core::domain::token_refresh::service::AsyncAuthRefreshAdapter + Send,
    D: oar_core::storage::postgres::audit_outbox_worker::AuditOutboxDispatcher + Send,
    C: FnMut() -> u64 + Send + 'static,
{
    type Report = PostgresTenantMaintenanceReport;
    type Error = PostgresRepositoryError;

    fn tick(&mut self) -> RuntimeTickFuture<'_, Self::Report, Self::Error> {
        Box::pin(async move { self.run_once().await })
    }

    fn safe_error(error: &Self::Error) -> String {
        match error {
            PostgresRepositoryError::Sqlx(_) => {
                "tenant_maintenance_runtime_tick_failed: postgres_query_failed".to_string()
            }
            PostgresRepositoryError::UnknownActionStatus(_) => {
                "tenant_maintenance_runtime_tick_failed: unknown_action_status".to_string()
            }
            PostgresRepositoryError::UnknownAuditActorKind(_) => {
                "tenant_maintenance_runtime_tick_failed: unknown_audit_actor_kind".to_string()
            }
            PostgresRepositoryError::UnknownAuditEventType(_) => {
                "tenant_maintenance_runtime_tick_failed: unknown_audit_event_type".to_string()
            }
            PostgresRepositoryError::UnknownExecutionStatus(_) => {
                "tenant_maintenance_runtime_tick_failed: unknown_execution_status".to_string()
            }
            PostgresRepositoryError::UnknownDeviceEntryPoint(_) => {
                "tenant_maintenance_runtime_tick_failed: unknown_device_entry_point".to_string()
            }
            PostgresRepositoryError::UnknownDeviceSessionState(_) => {
                "tenant_maintenance_runtime_tick_failed: unknown_device_session_state".to_string()
            }
            PostgresRepositoryError::UnknownTokenGrantState(_) => {
                "tenant_maintenance_runtime_tick_failed: unknown_token_grant_state".to_string()
            }
            PostgresRepositoryError::UnknownTenantStatus(_) => {
                "tenant_maintenance_runtime_tick_failed: unknown_tenant_status".to_string()
            }
            PostgresRepositoryError::UnknownWorkspaceUserStatus(_) => {
                "tenant_maintenance_runtime_tick_failed: unknown_workspace_user_status".to_string()
            }
            PostgresRepositoryError::UnknownIdentityActorKind(_) => {
                "tenant_maintenance_runtime_tick_failed: unknown_identity_actor_kind".to_string()
            }
            PostgresRepositoryError::UnknownScopeBoundary(_) => {
                "tenant_maintenance_runtime_tick_failed: unknown_scope_boundary".to_string()
            }
            PostgresRepositoryError::UnknownEvidenceSourceKind(_) => {
                "tenant_maintenance_runtime_tick_failed: unknown_evidence_source_kind".to_string()
            }
            PostgresRepositoryError::UnknownEvidenceVisibilityScope(_) => {
                "tenant_maintenance_runtime_tick_failed: unknown_evidence_visibility_scope".to_string()
            }
            PostgresRepositoryError::UnknownProposedActionStatus(_) => {
                "tenant_maintenance_runtime_tick_failed: unknown_proposed_action_status".to_string()
            }
            PostgresRepositoryError::UnknownProposedActionKind(_) => {
                "tenant_maintenance_runtime_tick_failed: unknown_proposed_action_kind".to_string()
            }
            PostgresRepositoryError::UnknownRiskSeverity(_) => {
                "tenant_maintenance_runtime_tick_failed: unknown_risk_severity".to_string()
            }
            PostgresRepositoryError::UnknownProposedActionDecision(_) => {
                "tenant_maintenance_runtime_tick_failed: unknown_proposed_action_decision".to_string()
            }
            PostgresRepositoryError::UnknownReviewInboxItemStatus(_) => {
                "tenant_maintenance_runtime_tick_failed: unknown_review_inbox_item_status".to_string()
            }
            PostgresRepositoryError::UnknownSchedulerJobKind(_) => {
                "tenant_maintenance_runtime_tick_failed: unknown_scheduler_job_kind".to_string()
            }
            PostgresRepositoryError::UnknownSchedulerJobStatus(_) => {
                "tenant_maintenance_runtime_tick_failed: unknown_scheduler_job_status".to_string()
            }
            PostgresRepositoryError::UnsafeSchedulerJobErrorCode => {
                "tenant_maintenance_runtime_tick_failed: unsafe_scheduler_job_error_code".to_string()
            }
            PostgresRepositoryError::UnsafeAuditOutboxPayload => {
                "tenant_maintenance_runtime_tick_failed: unsafe_audit_outbox_payload".to_string()
            }
            PostgresRepositoryError::ActionNotConfirmed(_) => {
                "tenant_maintenance_runtime_tick_failed: action_not_confirmed".to_string()
            }
            PostgresRepositoryError::TenantMismatch { .. } => {
                "tenant_maintenance_runtime_tick_failed: tenant_mismatch".to_string()
            }
            PostgresRepositoryError::LarkIdentityActorExternalBindingConflict { .. } => {
                "tenant_maintenance_runtime_tick_failed: lark_identity_actor_external_binding_conflict"
                    .to_string()
            }
            PostgresRepositoryError::NegativeInteger { .. } => {
                "tenant_maintenance_runtime_tick_failed: negative_integer".to_string()
            }
            PostgresRepositoryError::Json(_) => {
                "tenant_maintenance_runtime_tick_failed: invalid_json_payload".to_string()
            }
            PostgresRepositoryError::TokenRefreshDecisionBridge(_) => {
                "tenant_maintenance_runtime_tick_failed: token_refresh_decision_bridge_failed"
                    .to_string()
            }
            PostgresRepositoryError::InvalidOperationStatusTransition { .. } => {
                "tenant_maintenance_runtime_tick_failed: invalid_operation_status_transition"
                    .to_string()
            }
            PostgresRepositoryError::UnknownOperationIdempotencyKey(_) => {
                "tenant_maintenance_runtime_tick_failed: unknown_operation_idempotency_key"
                    .to_string()
            }
            PostgresRepositoryError::TokenRefreshPlanMismatch { .. } => {
                "tenant_maintenance_runtime_tick_failed: token_refresh_plan_mismatch".to_string()
            }
            PostgresRepositoryError::ReviewDecisionRequestMismatch { .. } => {
                "tenant_maintenance_runtime_tick_failed: review_decision_request_mismatch"
                    .to_string()
            }
            PostgresRepositoryError::MissingConfirmedActionForDecision => {
                "tenant_maintenance_runtime_tick_failed: missing_confirmed_action_for_decision"
                    .to_string()
            }
            PostgresRepositoryError::MissingConfirmedAtForDecision => {
                "tenant_maintenance_runtime_tick_failed: missing_confirmed_at_for_decision"
                    .to_string()
            }
            PostgresRepositoryError::MissingOperationIdForDecision => {
                "tenant_maintenance_runtime_tick_failed: missing_operation_id_for_decision"
                    .to_string()
            }
            PostgresRepositoryError::UnexpectedConfirmedActionForDecision => {
                "tenant_maintenance_runtime_tick_failed: unexpected_confirmed_action_for_decision"
                    .to_string()
            }
            PostgresRepositoryError::UnexpectedOperationIdForDecision => {
                "tenant_maintenance_runtime_tick_failed: unexpected_operation_id_for_decision"
                    .to_string()
            }
        }
    }
}

#[cfg(test)]
struct FnRuntimeTick<F> {
    tick_fn: F,
}

#[cfg(test)]
impl<F> FnRuntimeTick<F> {
    fn new(tick_fn: F) -> Self {
        Self { tick_fn }
    }
}

#[cfg(test)]
impl<F, Fut, R, E> RuntimeTick for FnRuntimeTick<F>
where
    F: FnMut() -> Fut + Send,
    Fut: Future<Output = Result<R, E>> + Send,
    R: Send + 'static,
    E: Error + Send + Sync + 'static,
{
    type Report = R;
    type Error = E;

    fn tick(&mut self) -> RuntimeTickFuture<'_, Self::Report, Self::Error> {
        Box::pin(async move { (self.tick_fn)().await })
    }

    fn safe_error(error: &Self::Error) -> String {
        error.to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use tokio::time;

    use super::*;

    #[derive(Debug, thiserror::Error)]
    #[error("{0}")]
    struct TestError(&'static str);

    fn assert_send<T: Send>() {}

    #[test]
    fn runtime_tick_future_is_send() {
        assert_send::<RuntimeTickFuture<'static, (), TestError>>();
    }

    #[tokio::test(start_paused = true)]
    async fn interval_triggers_multiple_ticks() {
        let hits = Arc::new(AtomicUsize::new(0));
        let hits_for_tick = Arc::clone(&hits);
        let cancellation = CancellationToken::new();
        let cancellation_for_tick = cancellation.clone();
        let runtime_tick = FnRuntimeTick::new(move || {
            let hits_for_tick = Arc::clone(&hits_for_tick);
            let cancellation_for_tick = cancellation_for_tick.clone();
            async move {
                let count = hits_for_tick.fetch_add(1, Ordering::SeqCst) + 1;
                if count >= 3 {
                    cancellation_for_tick.cancel();
                }
                Ok::<usize, TestError>(count)
            }
        });
        let mut runtime = TenantMaintenanceRuntime::try_new(
            TenantMaintenanceRuntimeConfig {
                tick_interval: Duration::from_secs(10),
            },
            runtime_tick,
        )
        .expect("test runtime config should be valid");

        let (report, _) = tokio::join!(runtime.run_until_cancelled(&cancellation), async {
            time::advance(Duration::from_secs(31)).await;
        });

        assert_eq!(hits.load(Ordering::SeqCst), 3);
        assert_eq!(report.successful_ticks, 3);
        assert_eq!(report.failed_ticks, 0);
    }

    #[tokio::test(start_paused = true)]
    async fn cancellation_stops_runtime() {
        let hits = Arc::new(AtomicUsize::new(0));
        let hits_for_tick = Arc::clone(&hits);
        let cancellation = CancellationToken::new();
        let runtime_tick = FnRuntimeTick::new(move || {
            let hits_for_tick = Arc::clone(&hits_for_tick);
            async move {
                hits_for_tick.fetch_add(1, Ordering::SeqCst);
                Ok::<(), TestError>(())
            }
        });
        let mut runtime = TenantMaintenanceRuntime::try_new(
            TenantMaintenanceRuntimeConfig {
                tick_interval: Duration::from_secs(10),
            },
            runtime_tick,
        )
        .expect("test runtime config should be valid");

        let (report, _) = tokio::join!(runtime.run_until_cancelled(&cancellation), async {
            time::advance(Duration::from_millis(1)).await;
            cancellation.cancel();
            time::advance(Duration::from_secs(1)).await;
        });

        assert!(report.cancelled);
    }

    #[tokio::test(start_paused = true)]
    async fn tick_error_is_reported_and_runtime_continues() {
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_for_tick = Arc::clone(&calls);
        let cancellation = CancellationToken::new();
        let cancellation_for_tick = cancellation.clone();

        let runtime_tick = FnRuntimeTick::new(move || {
            let calls_for_tick = Arc::clone(&calls_for_tick);
            let cancellation_for_tick = cancellation_for_tick.clone();
            async move {
                let call = calls_for_tick.fetch_add(1, Ordering::SeqCst) + 1;
                if call >= 3 {
                    cancellation_for_tick.cancel();
                }
                if call == 1 {
                    return Err(TestError("first_failed"));
                }
                Ok::<usize, TestError>(call)
            }
        });

        let mut runtime = TenantMaintenanceRuntime::try_new(
            TenantMaintenanceRuntimeConfig {
                tick_interval: Duration::from_secs(10),
            },
            runtime_tick,
        )
        .expect("test runtime config should be valid");
        let (report, _) = tokio::join!(runtime.run_until_cancelled(&cancellation), async {
            time::advance(Duration::from_secs(31)).await;
        });

        assert_eq!(report.failed_ticks, 1);
        assert_eq!(report.successful_ticks, 2);
        assert!(matches!(
            report.last_tick,
            Some(RuntimeTickReport::Succeeded(3))
        ));
    }

    #[tokio::test(start_paused = true)]
    async fn already_cancelled_token_does_not_tick() {
        let hits = Arc::new(AtomicUsize::new(0));
        let hits_for_tick = Arc::clone(&hits);
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let runtime_tick = FnRuntimeTick::new(move || {
            let hits_for_tick = Arc::clone(&hits_for_tick);
            async move {
                hits_for_tick.fetch_add(1, Ordering::SeqCst);
                Ok::<(), TestError>(())
            }
        });
        let mut runtime = TenantMaintenanceRuntime::try_new(
            TenantMaintenanceRuntimeConfig {
                tick_interval: Duration::from_secs(10),
            },
            runtime_tick,
        )
        .expect("test runtime config should be valid");

        let report = runtime.run_until_cancelled(&cancellation).await;

        assert_eq!(hits.load(Ordering::SeqCst), 0);
        assert_eq!(report.successful_ticks, 0);
        assert_eq!(report.failed_ticks, 0);
        assert_eq!(report.last_tick, None);
        assert!(report.cancelled);
    }

    #[tokio::test(start_paused = true)]
    async fn failed_last_tick_reports_safe_error_without_stopping() {
        let cancellation = CancellationToken::new();
        let cancellation_for_tick = cancellation.clone();
        let runtime_tick = FnRuntimeTick::new(move || {
            let cancellation_for_tick = cancellation_for_tick.clone();
            async move {
                cancellation_for_tick.cancel();
                Err::<usize, TestError>(TestError("safe_failure"))
            }
        });

        let mut runtime = TenantMaintenanceRuntime::try_new(
            TenantMaintenanceRuntimeConfig {
                tick_interval: Duration::from_secs(10),
            },
            runtime_tick,
        )
        .expect("test runtime config should be valid");
        let (report, _) = tokio::join!(runtime.run_until_cancelled(&cancellation), async {
            time::advance(Duration::from_secs(1)).await;
        });

        assert_eq!(report.failed_ticks, 1);
        assert_eq!(report.successful_ticks, 0);
        assert!(matches!(
            report.last_tick,
            Some(RuntimeTickReport::Failed(RuntimeTickFailure { safe_error }))
                if safe_error == "safe_failure"
        ));
    }

    #[test]
    fn zero_interval_is_rejected() {
        let result = TenantMaintenanceRuntimeConfig {
            tick_interval: Duration::ZERO,
        }
        .validate();
        assert_eq!(
            result,
            Err(TenantMaintenanceRuntimeConfigValidationError::ZeroTickInterval)
        );
    }
}

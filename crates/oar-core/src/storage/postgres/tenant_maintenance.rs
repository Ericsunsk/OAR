use sqlx::PgPool;
use std::fmt;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

type MaintenanceClock = Box<dyn FnMut() -> u64 + Send>;

use super::audit_outbox_worker::{
    AuditOutboxDispatcher, AuditOutboxDrainConfig, AuditOutboxDrainReport,
    PostgresAuditOutboxWorker,
};
use super::repository_safe_error::postgres_repository_safe_error;
use super::token_refresh_scheduler::{
    PostgresTokenRefreshScheduledSweep, TokenRefreshScheduledSweepConfig,
    TokenRefreshScheduledSweepReport,
};
use super::{
    PostgresAuditEventRepository, PostgresRepositoryError, PostgresSchedulerJobRepository,
    PostgresTokenRefreshSweep,
};
use crate::action::audit_event::AuditActor;
use crate::domain::token_refresh::service::AsyncAuthRefreshAdapter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostgresTenantMaintenanceConfig {
    pub tenant_id: String,
    pub lease_id: String,
    pub audit_stream: String,
    pub scheduled_lease_ms: u64,
    pub scheduled_retry_delay_ms: u64,
    pub scheduled_next_run_delay_ms: u64,
    pub scheduled_backlog_next_run_delay_ms: u64,
    pub scheduled_due_before_ms: u64,
    pub scheduled_limit: u32,
    pub scheduled_audit_trace_id: String,
    pub scheduled_audit_sequence_start: u64,
    pub scheduled_actor: AuditActor,
    pub scheduled_workspace_id: Option<String>,
    pub outbox_batch_limit: i64,
    pub outbox_lease_ms: u64,
    pub outbox_retry_delay_ms: u64,
    pub outbox_max_attempts: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PostgresTenantMaintenanceConfigValidationError {
    EmptyField(&'static str),
    NonPositiveField(&'static str),
}

impl fmt::Display for PostgresTenantMaintenanceConfigValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField(field) => {
                write!(f, "tenant_maintenance_config_invalid: {field}_empty")
            }
            Self::NonPositiveField(field) => {
                write!(f, "tenant_maintenance_config_invalid: {field}_non_positive")
            }
        }
    }
}

impl std::error::Error for PostgresTenantMaintenanceConfigValidationError {}

impl PostgresTenantMaintenanceConfig {
    pub fn validate(&self) -> Result<(), PostgresTenantMaintenanceConfigValidationError> {
        validate_non_empty("tenant_id", &self.tenant_id)?;
        validate_non_empty("lease_id", &self.lease_id)?;
        validate_non_empty("audit_stream", &self.audit_stream)?;
        validate_non_empty("scheduled_audit_trace_id", &self.scheduled_audit_trace_id)?;

        validate_non_zero_u64("scheduled_lease_ms", self.scheduled_lease_ms)?;
        validate_non_zero_u64("scheduled_retry_delay_ms", self.scheduled_retry_delay_ms)?;
        validate_non_zero_u64(
            "scheduled_next_run_delay_ms",
            self.scheduled_next_run_delay_ms,
        )?;
        validate_non_zero_u64(
            "scheduled_backlog_next_run_delay_ms",
            self.scheduled_backlog_next_run_delay_ms,
        )?;
        validate_non_zero_u32("scheduled_limit", self.scheduled_limit)?;

        validate_positive_i64("outbox_batch_limit", self.outbox_batch_limit)?;
        validate_non_zero_u64("outbox_lease_ms", self.outbox_lease_ms)?;
        validate_non_zero_u64("outbox_retry_delay_ms", self.outbox_retry_delay_ms)?;
        validate_non_zero_u32("outbox_max_attempts", self.outbox_max_attempts)?;

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostgresTenantMaintenanceReport {
    pub scheduled_sweep: PostgresTenantMaintenanceStage<TokenRefreshScheduledSweepReport>,
    pub outbox_drain: PostgresTenantMaintenanceStage<AuditOutboxDrainReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PostgresTenantMaintenanceStage<T> {
    Succeeded(T),
    Failed(PostgresTenantMaintenanceStageFailure),
}

impl<T> PostgresTenantMaintenanceStage<T> {
    pub fn succeeded(&self) -> Option<&T> {
        match self {
            Self::Succeeded(value) => Some(value),
            Self::Failed(_) => None,
        }
    }

    pub fn failed(&self) -> Option<&PostgresTenantMaintenanceStageFailure> {
        match self {
            Self::Succeeded(_) => None,
            Self::Failed(error) => Some(error),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostgresTenantMaintenanceStageFailure {
    pub safe_error: String,
}

pub struct PostgresTenantMaintenanceWorker<R, D, C = fn() -> u64>
where
    R: AsyncAuthRefreshAdapter,
    D: AuditOutboxDispatcher,
    C: FnMut() -> u64 + Send + 'static,
{
    scheduled_sweep: PostgresTokenRefreshScheduledSweep<R, MaintenanceClock>,
    outbox_drain: PostgresAuditOutboxWorker<D, MaintenanceClock>,
    _clock: PhantomData<C>,
}

impl<R, D, C> PostgresTenantMaintenanceWorker<R, D, C>
where
    R: AsyncAuthRefreshAdapter,
    D: AuditOutboxDispatcher,
    C: FnMut() -> u64 + Send + 'static,
{
    pub fn new(
        pool: PgPool,
        refresh_adapter: R,
        outbox_dispatcher: D,
        clock_ms: C,
        config: PostgresTenantMaintenanceConfig,
    ) -> Self {
        Self::try_new(pool, refresh_adapter, outbox_dispatcher, clock_ms, config)
            .expect("invalid tenant maintenance config")
    }

    pub fn try_new(
        pool: PgPool,
        refresh_adapter: R,
        outbox_dispatcher: D,
        clock_ms: C,
        config: PostgresTenantMaintenanceConfig,
    ) -> Result<Self, PostgresTenantMaintenanceConfigValidationError> {
        config.validate()?;

        let clock_ms = Arc::new(Mutex::new(clock_ms));
        let scheduled_clock = shared_clock(clock_ms.clone());
        let outbox_clock = shared_clock(clock_ms);

        let scheduled_sweep = PostgresTokenRefreshScheduledSweep::new(
            PostgresSchedulerJobRepository::new(pool.clone()),
            PostgresTokenRefreshSweep::new(pool.clone(), refresh_adapter),
            scheduled_clock,
            TokenRefreshScheduledSweepConfig {
                tenant_id: config.tenant_id.clone(),
                lease_id: config.lease_id.clone(),
                lease_ms: config.scheduled_lease_ms,
                retry_delay_ms: config.scheduled_retry_delay_ms,
                next_run_delay_ms: config.scheduled_next_run_delay_ms,
                backlog_next_run_delay_ms: config.scheduled_backlog_next_run_delay_ms,
                due_before_ms: config.scheduled_due_before_ms,
                limit: config.scheduled_limit,
                audit_trace_id: config.scheduled_audit_trace_id.clone(),
                audit_sequence_start: config.scheduled_audit_sequence_start,
                actor: config.scheduled_actor.clone(),
                workspace_id: config.scheduled_workspace_id.clone(),
            },
        );
        let outbox_drain = PostgresAuditOutboxWorker::new(
            PostgresAuditEventRepository::new(pool),
            outbox_dispatcher,
            outbox_clock,
            AuditOutboxDrainConfig {
                tenant_id: config.tenant_id,
                stream: config.audit_stream,
                batch_limit: config.outbox_batch_limit,
                lease_ms: config.outbox_lease_ms,
                retry_delay_ms: config.outbox_retry_delay_ms,
                max_attempts: config.outbox_max_attempts,
            },
        );
        Ok(Self {
            scheduled_sweep,
            outbox_drain,
            _clock: PhantomData,
        })
    }

    pub async fn run_once(
        &mut self,
    ) -> Result<PostgresTenantMaintenanceReport, PostgresRepositoryError> {
        let scheduled_sweep = match self.scheduled_sweep.run_once().await {
            Ok(report) => PostgresTenantMaintenanceStage::Succeeded(report),
            Err(error) => PostgresTenantMaintenanceStage::Failed(stage_failure(&error)),
        };
        let outbox_drain = match self.outbox_drain.drain_once().await {
            Ok(report) => PostgresTenantMaintenanceStage::Succeeded(report),
            Err(error) => PostgresTenantMaintenanceStage::Failed(stage_failure(&error)),
        };
        Ok(PostgresTenantMaintenanceReport {
            scheduled_sweep,
            outbox_drain,
        })
    }
}

fn validate_non_empty(
    field: &'static str,
    value: &str,
) -> Result<(), PostgresTenantMaintenanceConfigValidationError> {
    if value.trim().is_empty() {
        return Err(PostgresTenantMaintenanceConfigValidationError::EmptyField(
            field,
        ));
    }
    Ok(())
}

fn validate_non_zero_u64(
    field: &'static str,
    value: u64,
) -> Result<(), PostgresTenantMaintenanceConfigValidationError> {
    if value == 0 {
        return Err(PostgresTenantMaintenanceConfigValidationError::NonPositiveField(field));
    }
    Ok(())
}

fn validate_non_zero_u32(
    field: &'static str,
    value: u32,
) -> Result<(), PostgresTenantMaintenanceConfigValidationError> {
    if value == 0 {
        return Err(PostgresTenantMaintenanceConfigValidationError::NonPositiveField(field));
    }
    Ok(())
}

fn validate_positive_i64(
    field: &'static str,
    value: i64,
) -> Result<(), PostgresTenantMaintenanceConfigValidationError> {
    if value <= 0 {
        return Err(PostgresTenantMaintenanceConfigValidationError::NonPositiveField(field));
    }
    Ok(())
}

fn stage_failure(error: &PostgresRepositoryError) -> PostgresTenantMaintenanceStageFailure {
    PostgresTenantMaintenanceStageFailure {
        safe_error: postgres_repository_safe_error("tenant_maintenance_stage_failed", error),
    }
}

fn shared_clock<C>(clock: Arc<Mutex<C>>) -> MaintenanceClock
where
    C: FnMut() -> u64 + Send + 'static,
{
    Box::new(move || {
        let mut clock = clock.lock().expect("tenant maintenance clock mutex");
        (clock)()
    })
}

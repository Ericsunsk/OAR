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
    PostgresTenantRepository,
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
pub type RuntimeTenantDiscoveryFuture<'a, E> =
    Pin<Box<dyn Future<Output = Result<Vec<String>, E>> + Send + 'a>>;
pub type RuntimeTenantTickFactoryFuture<'a, T, E> =
    Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'a>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeRunReport<T> {
    pub successful_ticks: usize,
    pub failed_ticks: usize,
    pub last_tick: Option<RuntimeTickReport<T>>,
    pub cancelled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantRuntimeReport<T> {
    pub tenant_id: String,
    pub successful_ticks: usize,
    pub failed_ticks: usize,
    pub last_tick: Option<RuntimeTickReport<T>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantMaintenanceRegistryRunReport<T> {
    pub tenant_reports: Vec<TenantRuntimeReport<T>>,
    pub completed_rounds: usize,
    pub cancelled: bool,
}

pub trait RuntimeTick {
    type Report: Send + 'static;
    type Error: Error + Send + Sync + 'static;

    fn tick(&mut self) -> RuntimeTickFuture<'_, Self::Report, Self::Error>;
    fn safe_error(error: &Self::Error) -> String;
}

pub struct RuntimeTenantTick<T> {
    tenant_id: String,
    tick: T,
}

pub type RuntimeTenantReport<T> = TenantRuntimeReport<T>;
pub type RuntimeRegistryRunReport<T> = TenantMaintenanceRegistryRunReport<T>;

pub trait RuntimeTenantDiscovery {
    type Error: Error + Send + Sync + 'static;

    fn discover_tenant_ids(&mut self) -> RuntimeTenantDiscoveryFuture<'_, Self::Error>;
    fn safe_error(error: &Self::Error) -> String;
}

pub trait RuntimeTenantTickFactory<T>
where
    T: RuntimeTick,
{
    type Error: Error + Send + Sync + 'static;

    fn build_tick(&mut self, tenant_id: &str)
        -> RuntimeTenantTickFactoryFuture<'_, T, Self::Error>;
    fn safe_error(error: &Self::Error) -> String;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TenantMaintenanceRuntimeRegistryBuildError {
    InvalidRuntimeConfig(TenantMaintenanceRuntimeConfigValidationError),
    DiscoveryFailed {
        safe_error: String,
    },
    EmptyRegistry,
    EmptyTenantId,
    DuplicateTenantId(String),
    TickFactoryFailed {
        tenant_id: String,
        safe_error: String,
    },
}

impl fmt::Display for TenantMaintenanceRuntimeRegistryBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRuntimeConfig(error) => write!(f, "{error}"),
            Self::DiscoveryFailed { .. } => {
                write!(
                    f,
                    "tenant_maintenance_registry_build_failed: discovery_failed"
                )
            }
            Self::EmptyRegistry => write!(
                f,
                "tenant_maintenance_registry_build_failed: empty_registry"
            ),
            Self::EmptyTenantId => {
                write!(
                    f,
                    "tenant_maintenance_registry_build_failed: empty_tenant_id"
                )
            }
            Self::DuplicateTenantId(_) => {
                write!(
                    f,
                    "tenant_maintenance_registry_build_failed: duplicate_tenant_id"
                )
            }
            Self::TickFactoryFailed { .. } => {
                write!(
                    f,
                    "tenant_maintenance_registry_build_failed: tick_factory_failed"
                )
            }
        }
    }
}

impl Error for TenantMaintenanceRuntimeRegistryBuildError {}

pub struct TenantMaintenanceRuntimeRegistryBuilder {
    config: TenantMaintenanceRuntimeConfig,
}

impl TenantMaintenanceRuntimeRegistryBuilder {
    pub fn new(config: TenantMaintenanceRuntimeConfig) -> Self {
        Self { config }
    }

    pub async fn build<T, D, F>(
        self,
        discovery: &mut D,
        factory: &mut F,
    ) -> Result<TenantMaintenanceRuntimeRegistry<T>, TenantMaintenanceRuntimeRegistryBuildError>
    where
        T: RuntimeTick,
        D: RuntimeTenantDiscovery,
        F: RuntimeTenantTickFactory<T>,
    {
        self.config
            .validate()
            .map_err(TenantMaintenanceRuntimeRegistryBuildError::InvalidRuntimeConfig)?;

        let discovered = discovery.discover_tenant_ids().await.map_err(|error| {
            TenantMaintenanceRuntimeRegistryBuildError::DiscoveryFailed {
                safe_error: D::safe_error(&error),
            }
        })?;
        let tenant_ids = normalize_and_validate_tenant_ids(discovered)?;

        let mut ticks = Vec::with_capacity(tenant_ids.len());
        for tenant_id in tenant_ids {
            let tick = factory.build_tick(&tenant_id).await.map_err(|error| {
                TenantMaintenanceRuntimeRegistryBuildError::TickFactoryFailed {
                    tenant_id: tenant_id.clone(),
                    safe_error: F::safe_error(&error),
                }
            })?;
            ticks.push(RuntimeTenantTick::new(tenant_id, tick));
        }

        TenantMaintenanceRuntimeRegistry::try_new(self.config, ticks).map_err(|error| match error {
            TenantMaintenanceRuntimeRegistryValidationError::InvalidRuntimeConfig(inner) => {
                TenantMaintenanceRuntimeRegistryBuildError::InvalidRuntimeConfig(inner)
            }
            TenantMaintenanceRuntimeRegistryValidationError::EmptyRegistry => {
                TenantMaintenanceRuntimeRegistryBuildError::EmptyRegistry
            }
            TenantMaintenanceRuntimeRegistryValidationError::EmptyTenantId => {
                TenantMaintenanceRuntimeRegistryBuildError::EmptyTenantId
            }
            TenantMaintenanceRuntimeRegistryValidationError::DuplicateTenantId(tenant_id) => {
                TenantMaintenanceRuntimeRegistryBuildError::DuplicateTenantId(tenant_id)
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticRuntimeTenantDiscovery {
    tenant_ids: Vec<String>,
}

impl StaticRuntimeTenantDiscovery {
    pub fn new(tenant_ids: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            tenant_ids: tenant_ids.into_iter().map(Into::into).collect(),
        }
    }
}

impl RuntimeTenantDiscovery for StaticRuntimeTenantDiscovery {
    type Error = std::convert::Infallible;

    fn discover_tenant_ids(&mut self) -> RuntimeTenantDiscoveryFuture<'_, Self::Error> {
        let tenant_ids = self.tenant_ids.clone();
        Box::pin(async move { Ok(tenant_ids) })
    }

    fn safe_error(_error: &Self::Error) -> String {
        "tenant_maintenance_registry_build_failed: static_discovery_unreachable".to_string()
    }
}

pub struct PostgresRuntimeTenantDiscovery {
    repository: PostgresTenantRepository,
}

impl PostgresRuntimeTenantDiscovery {
    pub fn new(repository: PostgresTenantRepository) -> Self {
        Self { repository }
    }

    fn map_safe_error(error: &PostgresRepositoryError) -> String {
        postgres_repository_safe_error("tenant_discovery_failed", error)
    }
}

fn postgres_repository_safe_error(prefix: &str, error: &PostgresRepositoryError) -> String {
    format!(
        "{}: {}",
        prefix,
        postgres_repository_safe_error_reason(error)
    )
}

fn postgres_repository_safe_error_reason(error: &PostgresRepositoryError) -> &'static str {
    match error {
        PostgresRepositoryError::Sqlx(_) => "postgres_query_failed",
        PostgresRepositoryError::UnknownActionStatus(_) => "unknown_action_status",
        PostgresRepositoryError::UnknownAuditActorKind(_) => "unknown_audit_actor_kind",
        PostgresRepositoryError::UnknownAuditEventType(_) => "unknown_audit_event_type",
        PostgresRepositoryError::UnknownExecutionStatus(_) => "unknown_execution_status",
        PostgresRepositoryError::UnknownDeviceEntryPoint(_) => "unknown_device_entry_point",
        PostgresRepositoryError::UnknownDeviceSessionState(_) => "unknown_device_session_state",
        PostgresRepositoryError::UnknownTokenGrantState(_) => "unknown_token_grant_state",
        PostgresRepositoryError::UnknownTenantStatus(_) => "unknown_tenant_status",
        PostgresRepositoryError::UnknownWorkspaceUserStatus(_) => "unknown_workspace_user_status",
        PostgresRepositoryError::UnknownIdentityActorKind(_) => "unknown_identity_actor_kind",
        PostgresRepositoryError::UnknownScopeBoundary(_) => "unknown_scope_boundary",
        PostgresRepositoryError::UnknownEvidenceSourceKind(_) => "unknown_evidence_source_kind",
        PostgresRepositoryError::UnknownEvidenceVisibilityScope(_) => {
            "unknown_evidence_visibility_scope"
        }
        PostgresRepositoryError::UnknownProposedActionStatus(_) => "unknown_proposed_action_status",
        PostgresRepositoryError::UnknownProposedActionKind(_) => "unknown_proposed_action_kind",
        PostgresRepositoryError::UnknownRiskSeverity(_) => "unknown_risk_severity",
        PostgresRepositoryError::UnknownProposedActionDecision(_) => {
            "unknown_proposed_action_decision"
        }
        PostgresRepositoryError::UnknownReviewInboxItemStatus(_) => {
            "unknown_review_inbox_item_status"
        }
        PostgresRepositoryError::UnknownSchedulerJobKind(_) => "unknown_scheduler_job_kind",
        PostgresRepositoryError::UnknownSchedulerJobStatus(_) => "unknown_scheduler_job_status",
        PostgresRepositoryError::UnsafeSchedulerJobErrorCode => "unsafe_scheduler_job_error_code",
        PostgresRepositoryError::UnsafeAuditOutboxPayload => "unsafe_audit_outbox_payload",
        PostgresRepositoryError::ActionNotConfirmed(_) => "action_not_confirmed",
        PostgresRepositoryError::TenantMismatch { .. } => "tenant_mismatch",
        PostgresRepositoryError::LarkIdentityActorExternalBindingConflict { .. } => {
            "lark_identity_actor_external_binding_conflict"
        }
        PostgresRepositoryError::NegativeInteger { .. } => "negative_integer",
        PostgresRepositoryError::Json(_) => "invalid_json_payload",
        PostgresRepositoryError::TokenRefreshDecisionBridge(_) => {
            "token_refresh_decision_bridge_failed"
        }
        PostgresRepositoryError::InvalidOperationStatusTransition { .. } => {
            "invalid_operation_status_transition"
        }
        PostgresRepositoryError::UnknownOperationIdempotencyKey(_) => {
            "unknown_operation_idempotency_key"
        }
        PostgresRepositoryError::TokenRefreshPlanMismatch { .. } => "token_refresh_plan_mismatch",
        PostgresRepositoryError::ReviewDecisionRequestMismatch { .. } => {
            "review_decision_request_mismatch"
        }
        PostgresRepositoryError::MissingConfirmedActionForDecision => {
            "missing_confirmed_action_for_decision"
        }
        PostgresRepositoryError::MissingConfirmedAtForDecision => {
            "missing_confirmed_at_for_decision"
        }
        PostgresRepositoryError::MissingOperationIdForDecision => {
            "missing_operation_id_for_decision"
        }
        PostgresRepositoryError::UnexpectedConfirmedActionForDecision => {
            "unexpected_confirmed_action_for_decision"
        }
        PostgresRepositoryError::UnexpectedOperationIdForDecision => {
            "unexpected_operation_id_for_decision"
        }
    }
}

impl RuntimeTenantDiscovery for PostgresRuntimeTenantDiscovery {
    type Error = PostgresRepositoryError;

    fn discover_tenant_ids(&mut self) -> RuntimeTenantDiscoveryFuture<'_, Self::Error> {
        Box::pin(async move { self.repository.list_active_ids().await })
    }

    fn safe_error(error: &Self::Error) -> String {
        Self::map_safe_error(error)
    }
}

impl<T> RuntimeTenantTick<T> {
    pub fn new(tenant_id: impl Into<String>, tick: T) -> Self {
        Self {
            tenant_id: tenant_id.into(),
            tick,
        }
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TenantMaintenanceRuntimeRegistryValidationError {
    InvalidRuntimeConfig(TenantMaintenanceRuntimeConfigValidationError),
    EmptyRegistry,
    EmptyTenantId,
    DuplicateTenantId(String),
}

impl fmt::Display for TenantMaintenanceRuntimeRegistryValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRuntimeConfig(error) => write!(f, "{error}"),
            Self::EmptyRegistry => write!(f, "tenant_maintenance_registry_invalid: empty_registry"),
            Self::EmptyTenantId => {
                write!(f, "tenant_maintenance_registry_invalid: empty_tenant_id")
            }
            Self::DuplicateTenantId(_) => {
                write!(
                    f,
                    "tenant_maintenance_registry_invalid: duplicate_tenant_id"
                )
            }
        }
    }
}

impl Error for TenantMaintenanceRuntimeRegistryValidationError {}

struct TenantRuntimeState<T>
where
    T: RuntimeTick,
{
    tenant_id: String,
    tick: T,
    successful_ticks: usize,
    failed_ticks: usize,
    last_tick: Option<RuntimeTickReport<T::Report>>,
}

pub struct TenantMaintenanceRuntimeRegistry<T>
where
    T: RuntimeTick,
{
    config: TenantMaintenanceRuntimeConfig,
    tenants: Vec<TenantRuntimeState<T>>,
}

impl<T> TenantMaintenanceRuntimeRegistry<T>
where
    T: RuntimeTick,
{
    pub fn try_new(
        config: TenantMaintenanceRuntimeConfig,
        tenants: impl IntoIterator<Item = RuntimeTenantTick<T>>,
    ) -> Result<Self, TenantMaintenanceRuntimeRegistryValidationError> {
        config
            .validate()
            .map_err(TenantMaintenanceRuntimeRegistryValidationError::InvalidRuntimeConfig)?;

        let tenants: Vec<_> = tenants.into_iter().collect();
        validate_named_tenants(&tenants)?;

        Ok(Self {
            config,
            tenants: tenants
                .into_iter()
                .map(|tenant| TenantRuntimeState {
                    tenant_id: canonicalize_tenant_id(&tenant.tenant_id),
                    tick: tenant.tick,
                    successful_ticks: 0,
                    failed_ticks: 0,
                    last_tick: None,
                })
                .collect(),
        })
    }

    pub async fn run_once_round(&mut self) -> TenantMaintenanceRegistryRunReport<T::Report>
    where
        T::Report: Clone,
    {
        self.run_round(None).await;
        self.snapshot(1, false)
    }

    pub async fn run_until_cancelled(
        &mut self,
        cancellation: &CancellationToken,
    ) -> TenantMaintenanceRegistryRunReport<T::Report>
    where
        T::Report: Clone,
    {
        let mut interval = time::interval(self.config.tick_interval);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        let mut completed_rounds = 0usize;

        loop {
            if cancellation.is_cancelled() {
                info!("tenant maintenance registry cancelled");
                break;
            }

            tokio::select! {
                _ = cancellation.cancelled() => {
                    info!("tenant maintenance registry cancelled");
                    break;
                }
                _ = interval.tick() => {
                    if self.run_round(Some(cancellation)).await {
                        completed_rounds = completed_rounds.saturating_add(1);
                    } else {
                        info!("tenant maintenance registry cancelled during round");
                        break;
                    }
                }
            }
        }

        self.snapshot(completed_rounds, true)
    }

    async fn run_round(&mut self, cancellation: Option<&CancellationToken>) -> bool {
        for tenant in &mut self.tenants {
            if cancellation
                .map(CancellationToken::is_cancelled)
                .unwrap_or(false)
            {
                return false;
            }

            match tenant.tick.tick().await {
                Ok(report) => {
                    tenant.successful_ticks = tenant.successful_ticks.saturating_add(1);
                    tenant.last_tick = Some(RuntimeTickReport::Succeeded(report));
                }
                Err(error) => {
                    tenant.failed_ticks = tenant.failed_ticks.saturating_add(1);
                    let safe_error = T::safe_error(&error);
                    warn!(
                        tenant_id = %tenant.tenant_id,
                        safe_error = %safe_error,
                        "tenant maintenance registry tick failed"
                    );
                    tenant.last_tick =
                        Some(RuntimeTickReport::Failed(RuntimeTickFailure { safe_error }));
                }
            }
        }

        true
    }

    fn snapshot(
        &self,
        completed_rounds: usize,
        cancelled: bool,
    ) -> TenantMaintenanceRegistryRunReport<T::Report>
    where
        T::Report: Clone,
    {
        TenantMaintenanceRegistryRunReport {
            tenant_reports: self
                .tenants
                .iter()
                .map(|tenant| TenantRuntimeReport {
                    tenant_id: tenant.tenant_id.clone(),
                    successful_ticks: tenant.successful_ticks,
                    failed_ticks: tenant.failed_ticks,
                    last_tick: tenant.last_tick.clone(),
                })
                .collect(),
            completed_rounds,
            cancelled,
        }
    }
}

fn validate_named_tenants<T>(
    tenants: &[RuntimeTenantTick<T>],
) -> Result<(), TenantMaintenanceRuntimeRegistryValidationError> {
    validate_tenant_ids(
        tenants
            .iter()
            .map(|tenant| tenant.tenant_id.clone())
            .collect::<Vec<_>>(),
    )
    .map(|_| ())
    .map_err(|error| match error {
        TenantIdValidationError::EmptyRegistry => {
            TenantMaintenanceRuntimeRegistryValidationError::EmptyRegistry
        }
        TenantIdValidationError::EmptyTenantId => {
            TenantMaintenanceRuntimeRegistryValidationError::EmptyTenantId
        }
        TenantIdValidationError::DuplicateTenantId(tenant_id) => {
            TenantMaintenanceRuntimeRegistryValidationError::DuplicateTenantId(tenant_id)
        }
    })
}

fn canonicalize_tenant_id(tenant_id: &str) -> String {
    tenant_id.trim().to_string()
}

fn normalize_and_validate_tenant_ids(
    tenant_ids: Vec<String>,
) -> Result<Vec<String>, TenantMaintenanceRuntimeRegistryBuildError> {
    validate_tenant_ids(tenant_ids).map_err(|error| match error {
        TenantIdValidationError::EmptyRegistry => {
            TenantMaintenanceRuntimeRegistryBuildError::EmptyRegistry
        }
        TenantIdValidationError::EmptyTenantId => {
            TenantMaintenanceRuntimeRegistryBuildError::EmptyTenantId
        }
        TenantIdValidationError::DuplicateTenantId(tenant_id) => {
            TenantMaintenanceRuntimeRegistryBuildError::DuplicateTenantId(tenant_id)
        }
    })
}

enum TenantIdValidationError {
    EmptyRegistry,
    EmptyTenantId,
    DuplicateTenantId(String),
}

fn validate_tenant_ids(tenant_ids: Vec<String>) -> Result<Vec<String>, TenantIdValidationError> {
    use std::collections::HashSet;

    if tenant_ids.is_empty() {
        return Err(TenantIdValidationError::EmptyRegistry);
    }

    let mut seen = HashSet::new();
    let mut normalized = Vec::with_capacity(tenant_ids.len());
    for tenant_id in tenant_ids {
        let tenant_id = canonicalize_tenant_id(&tenant_id);
        if tenant_id.is_empty() {
            return Err(TenantIdValidationError::EmptyTenantId);
        }
        if !seen.insert(tenant_id.clone()) {
            return Err(TenantIdValidationError::DuplicateTenantId(tenant_id));
        }
        normalized.push(tenant_id);
    }

    Ok(normalized)
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
        postgres_repository_safe_error("tenant_maintenance_runtime_tick_failed", error)
    }
}

#[cfg(test)]
mod tests;

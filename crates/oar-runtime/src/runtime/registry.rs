use std::error::Error;
use std::fmt;

use tokio::time::{self, MissedTickBehavior};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use super::tenant_ids::{canonicalize_tenant_id, validate_tenant_ids, TenantIdValidationError};
use super::types::{
    RuntimeTenantTick, RuntimeTick, RuntimeTickFailure, RuntimeTickReport,
    TenantMaintenanceRegistryRunReport, TenantMaintenanceRuntimeConfig,
    TenantMaintenanceRuntimeConfigValidationError, TenantRuntimeReport,
};

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

impl TenantMaintenanceRuntimeRegistryValidationError {
    fn from_tenant_id_validation(error: TenantIdValidationError) -> Self {
        match error {
            TenantIdValidationError::EmptyRegistry => Self::EmptyRegistry,
            TenantIdValidationError::EmptyTenantId => Self::EmptyTenantId,
            TenantIdValidationError::DuplicateTenantId(tenant_id) => {
                Self::DuplicateTenantId(tenant_id)
            }
        }
    }
}

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
    .map_err(TenantMaintenanceRuntimeRegistryValidationError::from_tenant_id_validation)
}

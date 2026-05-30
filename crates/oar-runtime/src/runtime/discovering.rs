use std::marker::PhantomData;

use tokio::time::{self, MissedTickBehavior};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use super::registry::TenantMaintenanceRuntimeRegistry;
use super::tenant_ids::{validate_tenant_ids_allow_empty, TenantIdValidationError};
use super::types::{
    RuntimeTenantDiscovery, RuntimeTenantTick, RuntimeTenantTickFactory, RuntimeTick,
    RuntimeTickFailure, TenantMaintenanceRegistryRunReport, TenantMaintenanceRuntimeConfig,
    TenantMaintenanceRuntimeConfigValidationError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveringRuntimeRoundReport<T> {
    Succeeded(TenantMaintenanceRegistryRunReport<T>),
    Failed(RuntimeTickFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveringRuntimeRunReport<T> {
    pub successful_rounds: usize,
    pub failed_rounds: usize,
    pub last_round: Option<DiscoveringRuntimeRoundReport<T>>,
    pub cancelled: bool,
}

pub struct DiscoveringTenantMaintenanceRuntime<D, F, T>
where
    D: RuntimeTenantDiscovery,
    F: RuntimeTenantTickFactory<T>,
    T: RuntimeTick,
{
    config: TenantMaintenanceRuntimeConfig,
    discovery: D,
    factory: F,
    _tick: PhantomData<T>,
}

impl<D, F, T> DiscoveringTenantMaintenanceRuntime<D, F, T>
where
    D: RuntimeTenantDiscovery,
    F: RuntimeTenantTickFactory<T>,
    T: RuntimeTick,
{
    pub fn try_new(
        config: TenantMaintenanceRuntimeConfig,
        discovery: D,
        factory: F,
    ) -> Result<Self, TenantMaintenanceRuntimeConfigValidationError> {
        config.validate()?;
        Ok(Self {
            config,
            discovery,
            factory,
            _tick: PhantomData,
        })
    }

    pub async fn run_once_round(&mut self) -> DiscoveringRuntimeRoundReport<T::Report>
    where
        T::Report: Clone,
    {
        let tenant_ids = match self.discovery.discover_tenant_ids().await {
            Ok(tenant_ids) => tenant_ids,
            Err(error) => {
                return DiscoveringRuntimeRoundReport::Failed(RuntimeTickFailure {
                    safe_error: D::safe_error(&error),
                });
            }
        };
        let tenant_ids = match validate_tenant_ids_allow_empty(tenant_ids) {
            Ok(tenant_ids) => tenant_ids,
            Err(error) => {
                return DiscoveringRuntimeRoundReport::Failed(RuntimeTickFailure {
                    safe_error: tenant_id_validation_safe_error(error),
                });
            }
        };

        let mut ticks = Vec::with_capacity(tenant_ids.len());
        for tenant_id in tenant_ids {
            let tick = match self.factory.build_tick(&tenant_id).await {
                Ok(tick) => tick,
                Err(error) => {
                    return DiscoveringRuntimeRoundReport::Failed(RuntimeTickFailure {
                        safe_error: F::safe_error(&error),
                    });
                }
            };
            ticks.push(RuntimeTenantTick::new(tenant_id, tick));
        }

        if ticks.is_empty() {
            return DiscoveringRuntimeRoundReport::Succeeded(TenantMaintenanceRegistryRunReport {
                tenant_reports: Vec::new(),
                completed_rounds: 1,
                cancelled: false,
            });
        }

        let mut registry =
            match TenantMaintenanceRuntimeRegistry::try_new(self.config.clone(), ticks) {
                Ok(registry) => registry,
                Err(error) => {
                    return DiscoveringRuntimeRoundReport::Failed(RuntimeTickFailure {
                        safe_error: error.to_string(),
                    });
                }
            };
        DiscoveringRuntimeRoundReport::Succeeded(registry.run_once_round().await)
    }

    pub async fn run_until_cancelled(
        &mut self,
        cancellation: &CancellationToken,
    ) -> DiscoveringRuntimeRunReport<T::Report>
    where
        T::Report: Clone,
    {
        let mut interval = time::interval(self.config.tick_interval);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        let mut successful_rounds = 0usize;
        let mut failed_rounds = 0usize;
        let mut last_round = None;

        loop {
            if cancellation.is_cancelled() {
                info!("discovering tenant maintenance runtime cancelled");
                break;
            }

            tokio::select! {
                _ = cancellation.cancelled() => {
                    info!("discovering tenant maintenance runtime cancelled");
                    break;
                }
                _ = interval.tick() => {
                    let round = self.run_once_round().await;
                    match &round {
                        DiscoveringRuntimeRoundReport::Succeeded(_) => {
                            successful_rounds = successful_rounds.saturating_add(1);
                        }
                        DiscoveringRuntimeRoundReport::Failed(failure) => {
                            failed_rounds = failed_rounds.saturating_add(1);
                            warn!(
                                safe_error = %failure.safe_error,
                                "discovering tenant maintenance runtime round failed"
                            );
                        }
                    }
                    last_round = Some(round);
                }
            }
        }

        DiscoveringRuntimeRunReport {
            successful_rounds,
            failed_rounds,
            last_round,
            cancelled: true,
        }
    }
}

fn tenant_id_validation_safe_error(error: TenantIdValidationError) -> String {
    match error {
        TenantIdValidationError::EmptyRegistry => {
            "tenant_maintenance_discovery_invalid: empty_registry".to_string()
        }
        TenantIdValidationError::EmptyTenantId => {
            "tenant_maintenance_discovery_invalid: empty_tenant_id".to_string()
        }
        TenantIdValidationError::DuplicateTenantId(_) => {
            "tenant_maintenance_discovery_invalid: duplicate_tenant_id".to_string()
        }
    }
}

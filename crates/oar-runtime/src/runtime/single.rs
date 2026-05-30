use tokio::time::{self, MissedTickBehavior};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use super::types::{
    RuntimeRunReport, RuntimeTick, RuntimeTickFailure, RuntimeTickReport,
    TenantMaintenanceRuntimeConfig, TenantMaintenanceRuntimeConfigValidationError,
};

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

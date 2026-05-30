use oar_core::storage::postgres::{
    postgres_repository_safe_error_reason, PostgresRepositoryError,
    PostgresTenantMaintenanceReport, PostgresTenantMaintenanceWorker, PostgresTenantRepository,
};

use super::types::{
    RuntimeTenantDiscovery, RuntimeTenantDiscoveryFuture, RuntimeTick, RuntimeTickFuture,
};

pub struct PostgresRuntimeTenantDiscovery {
    repository: PostgresTenantRepository,
}

impl PostgresRuntimeTenantDiscovery {
    pub fn new(repository: PostgresTenantRepository) -> Self {
        Self { repository }
    }

    pub(super) fn map_safe_error(error: &PostgresRepositoryError) -> String {
        format!(
            "tenant_discovery_failed: {}",
            postgres_repository_safe_error_reason(error)
        )
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
        format!(
            "tenant_maintenance_runtime_tick_failed: {}",
            postgres_repository_safe_error_reason(error)
        )
    }
}

mod builder;
mod discovering;
mod postgres;
mod registry;
mod single;
mod tenant_ids;
mod types;

pub use builder::{
    StaticRuntimeTenantDiscovery, TenantMaintenanceRuntimeRegistryBuildError,
    TenantMaintenanceRuntimeRegistryBuilder,
};
pub use discovering::{
    DiscoveringRuntimeRoundReport, DiscoveringRuntimeRunReport, DiscoveringTenantMaintenanceRuntime,
};
pub use postgres::PostgresRuntimeTenantDiscovery;
pub use registry::{
    TenantMaintenanceRuntimeRegistry, TenantMaintenanceRuntimeRegistryValidationError,
};
pub use single::TenantMaintenanceRuntime;
pub use types::{
    RuntimeRegistryRunReport, RuntimeRunReport, RuntimeTenantDiscovery,
    RuntimeTenantDiscoveryFuture, RuntimeTenantReport, RuntimeTenantTick, RuntimeTenantTickFactory,
    RuntimeTenantTickFactoryFuture, RuntimeTick, RuntimeTickFailure, RuntimeTickFuture,
    RuntimeTickReport, TenantMaintenanceRegistryRunReport, TenantMaintenanceRuntimeConfig,
    TenantMaintenanceRuntimeConfigValidationError, TenantRuntimeReport,
};

#[cfg(test)]
mod tests;

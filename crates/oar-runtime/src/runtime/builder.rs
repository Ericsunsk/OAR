use std::convert::Infallible;
use std::error::Error;
use std::fmt;

use super::registry::{
    TenantMaintenanceRuntimeRegistry, TenantMaintenanceRuntimeRegistryValidationError,
};
use super::tenant_ids::{validate_tenant_ids, TenantIdValidationError};
use super::types::{
    RuntimeTenantDiscovery, RuntimeTenantDiscoveryFuture, RuntimeTenantTick,
    RuntimeTenantTickFactory, RuntimeTick, TenantMaintenanceRuntimeConfig,
    TenantMaintenanceRuntimeConfigValidationError,
};

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

impl From<TenantMaintenanceRuntimeRegistryValidationError>
    for TenantMaintenanceRuntimeRegistryBuildError
{
    fn from(error: TenantMaintenanceRuntimeRegistryValidationError) -> Self {
        match error {
            TenantMaintenanceRuntimeRegistryValidationError::InvalidRuntimeConfig(inner) => {
                Self::InvalidRuntimeConfig(inner)
            }
            TenantMaintenanceRuntimeRegistryValidationError::EmptyRegistry => Self::EmptyRegistry,
            TenantMaintenanceRuntimeRegistryValidationError::EmptyTenantId => Self::EmptyTenantId,
            TenantMaintenanceRuntimeRegistryValidationError::DuplicateTenantId(tenant_id) => {
                Self::DuplicateTenantId(tenant_id)
            }
        }
    }
}

impl TenantMaintenanceRuntimeRegistryBuildError {
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

        TenantMaintenanceRuntimeRegistry::try_new(self.config, ticks).map_err(Into::into)
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
    type Error = Infallible;

    fn discover_tenant_ids(&mut self) -> RuntimeTenantDiscoveryFuture<'_, Self::Error> {
        let tenant_ids = self.tenant_ids.clone();
        Box::pin(async move { Ok(tenant_ids) })
    }

    fn safe_error(_error: &Self::Error) -> String {
        "tenant_maintenance_registry_build_failed: static_discovery_unreachable".to_string()
    }
}

fn normalize_and_validate_tenant_ids(
    tenant_ids: Vec<String>,
) -> Result<Vec<String>, TenantMaintenanceRuntimeRegistryBuildError> {
    validate_tenant_ids(tenant_ids)
        .map_err(TenantMaintenanceRuntimeRegistryBuildError::from_tenant_id_validation)
}

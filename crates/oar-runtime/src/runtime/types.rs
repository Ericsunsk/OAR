use std::error::Error;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

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
    pub(super) tenant_id: String,
    pub(super) tick: T,
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

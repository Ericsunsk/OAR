use std::collections::VecDeque;
use std::error::Error;
use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use super::super::{
    RuntimeTenantDiscovery, RuntimeTenantDiscoveryFuture, RuntimeTenantTickFactory,
    RuntimeTenantTickFactoryFuture, RuntimeTick, RuntimeTickFuture, TenantMaintenanceRuntimeConfig,
};

pub(super) struct FnRuntimeTick<F> {
    tick_fn: F,
}

impl<F> FnRuntimeTick<F> {
    pub(super) fn new(tick_fn: F) -> Self {
        Self { tick_fn }
    }
}

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

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub(super) struct TestError(pub(super) &'static str);

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub(super) struct DiscoveryTestError(pub(super) &'static str);

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub(super) struct FactoryTestError(pub(super) &'static str);

pub(super) struct RegistryTestTick {
    calls: Arc<AtomicUsize>,
    outcome: RegistryTestOutcome,
    cancellation: Option<CancellationToken>,
}

pub(super) struct FailingDiscovery;

impl RuntimeTenantDiscovery for FailingDiscovery {
    type Error = DiscoveryTestError;

    fn discover_tenant_ids(&mut self) -> RuntimeTenantDiscoveryFuture<'_, Self::Error> {
        Box::pin(async { Err(DiscoveryTestError("discovery_raw_error")) })
    }

    fn safe_error(_error: &Self::Error) -> String {
        "tenant_discovery_failed".to_string()
    }
}

pub(super) struct QueueFactory {
    outcomes: VecDeque<Result<RegistryTestTick, FactoryTestError>>,
}

impl QueueFactory {
    pub(super) fn new(outcomes: Vec<Result<RegistryTestTick, FactoryTestError>>) -> Self {
        Self {
            outcomes: outcomes.into_iter().collect(),
        }
    }
}

impl RuntimeTenantTickFactory<RegistryTestTick> for QueueFactory {
    type Error = FactoryTestError;

    fn build_tick(
        &mut self,
        _tenant_id: &str,
    ) -> RuntimeTenantTickFactoryFuture<'_, RegistryTestTick, Self::Error> {
        let next = self
            .outcomes
            .pop_front()
            .expect("test factory should have enough queued outcomes");
        Box::pin(async move { next })
    }

    fn safe_error(_error: &Self::Error) -> String {
        "tenant_tick_factory_failed".to_string()
    }
}

pub(super) enum RegistryTestOutcome {
    Succeeded(usize),
    Failed(&'static str),
}

impl RegistryTestTick {
    pub(super) fn succeeded(calls: Arc<AtomicUsize>, report: usize) -> Self {
        Self {
            calls,
            outcome: RegistryTestOutcome::Succeeded(report),
            cancellation: None,
        }
    }

    pub(super) fn failed(calls: Arc<AtomicUsize>, safe_error: &'static str) -> Self {
        Self {
            calls,
            outcome: RegistryTestOutcome::Failed(safe_error),
            cancellation: None,
        }
    }

    pub(super) fn with_cancellation(mut self, cancellation: CancellationToken) -> Self {
        self.cancellation = Some(cancellation);
        self
    }
}

impl RuntimeTick for RegistryTestTick {
    type Report = usize;
    type Error = TestError;

    fn tick(&mut self) -> RuntimeTickFuture<'_, Self::Report, Self::Error> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if let Some(cancellation) = &self.cancellation {
                cancellation.cancel();
            }
            match self.outcome {
                RegistryTestOutcome::Succeeded(report) => Ok(report),
                RegistryTestOutcome::Failed(safe_error) => Err(TestError(safe_error)),
            }
        })
    }

    fn safe_error(error: &Self::Error) -> String {
        error.to_string()
    }
}

pub(super) fn assert_send<T: Send>() {}

pub(super) fn runtime_config() -> TenantMaintenanceRuntimeConfig {
    TenantMaintenanceRuntimeConfig {
        tick_interval: Duration::from_secs(10),
    }
}

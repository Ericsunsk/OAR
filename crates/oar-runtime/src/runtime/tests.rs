use std::collections::VecDeque;
use std::error::Error;
use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use oar_core::storage::postgres::PostgresRepositoryError;
use tokio::time;
use tokio_util::sync::CancellationToken;

use super::postgres::{postgres_repository_safe_error, postgres_repository_safe_error_reason};
use super::*;

struct FnRuntimeTick<F> {
    tick_fn: F,
}

impl<F> FnRuntimeTick<F> {
    fn new(tick_fn: F) -> Self {
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
struct TestError(&'static str);

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct DiscoveryTestError(&'static str);

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct FactoryTestError(&'static str);

struct RegistryTestTick {
    calls: Arc<AtomicUsize>,
    outcome: RegistryTestOutcome,
    cancellation: Option<CancellationToken>,
}

struct FailingDiscovery;

impl RuntimeTenantDiscovery for FailingDiscovery {
    type Error = DiscoveryTestError;

    fn discover_tenant_ids(&mut self) -> RuntimeTenantDiscoveryFuture<'_, Self::Error> {
        Box::pin(async { Err(DiscoveryTestError("discovery_raw_error")) })
    }

    fn safe_error(_error: &Self::Error) -> String {
        "tenant_discovery_failed".to_string()
    }
}

struct QueueFactory {
    outcomes: VecDeque<Result<RegistryTestTick, FactoryTestError>>,
}

impl QueueFactory {
    fn new(outcomes: Vec<Result<RegistryTestTick, FactoryTestError>>) -> Self {
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

enum RegistryTestOutcome {
    Succeeded(usize),
    Failed(&'static str),
}

impl RegistryTestTick {
    fn succeeded(calls: Arc<AtomicUsize>, report: usize) -> Self {
        Self {
            calls,
            outcome: RegistryTestOutcome::Succeeded(report),
            cancellation: None,
        }
    }

    fn failed(calls: Arc<AtomicUsize>, safe_error: &'static str) -> Self {
        Self {
            calls,
            outcome: RegistryTestOutcome::Failed(safe_error),
            cancellation: None,
        }
    }

    fn with_cancellation(mut self, cancellation: CancellationToken) -> Self {
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

fn assert_send<T: Send>() {}

#[test]
fn runtime_tick_future_is_send() {
    assert_send::<RuntimeTickFuture<'static, (), TestError>>();
}

#[tokio::test(start_paused = true)]
async fn interval_triggers_multiple_ticks() {
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_for_tick = Arc::clone(&hits);
    let cancellation = CancellationToken::new();
    let cancellation_for_tick = cancellation.clone();
    let runtime_tick = FnRuntimeTick::new(move || {
        let hits_for_tick = Arc::clone(&hits_for_tick);
        let cancellation_for_tick = cancellation_for_tick.clone();
        async move {
            let count = hits_for_tick.fetch_add(1, Ordering::SeqCst) + 1;
            if count >= 3 {
                cancellation_for_tick.cancel();
            }
            Ok::<usize, TestError>(count)
        }
    });
    let mut runtime = TenantMaintenanceRuntime::try_new(
        TenantMaintenanceRuntimeConfig {
            tick_interval: Duration::from_secs(10),
        },
        runtime_tick,
    )
    .expect("test runtime config should be valid");

    let (report, _) = tokio::join!(runtime.run_until_cancelled(&cancellation), async {
        time::advance(Duration::from_secs(31)).await;
    });

    assert_eq!(hits.load(Ordering::SeqCst), 3);
    assert_eq!(report.successful_ticks, 3);
    assert_eq!(report.failed_ticks, 0);
}

#[tokio::test(start_paused = true)]
async fn cancellation_stops_runtime() {
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_for_tick = Arc::clone(&hits);
    let cancellation = CancellationToken::new();
    let runtime_tick = FnRuntimeTick::new(move || {
        let hits_for_tick = Arc::clone(&hits_for_tick);
        async move {
            hits_for_tick.fetch_add(1, Ordering::SeqCst);
            Ok::<(), TestError>(())
        }
    });
    let mut runtime = TenantMaintenanceRuntime::try_new(
        TenantMaintenanceRuntimeConfig {
            tick_interval: Duration::from_secs(10),
        },
        runtime_tick,
    )
    .expect("test runtime config should be valid");

    let (report, _) = tokio::join!(runtime.run_until_cancelled(&cancellation), async {
        time::advance(Duration::from_millis(1)).await;
        cancellation.cancel();
        time::advance(Duration::from_secs(1)).await;
    });

    assert!(report.cancelled);
}

#[tokio::test(start_paused = true)]
async fn tick_error_is_reported_and_runtime_continues() {
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_for_tick = Arc::clone(&calls);
    let cancellation = CancellationToken::new();
    let cancellation_for_tick = cancellation.clone();

    let runtime_tick = FnRuntimeTick::new(move || {
        let calls_for_tick = Arc::clone(&calls_for_tick);
        let cancellation_for_tick = cancellation_for_tick.clone();
        async move {
            let call = calls_for_tick.fetch_add(1, Ordering::SeqCst) + 1;
            if call >= 3 {
                cancellation_for_tick.cancel();
            }
            if call == 1 {
                return Err(TestError("first_failed"));
            }
            Ok::<usize, TestError>(call)
        }
    });

    let mut runtime = TenantMaintenanceRuntime::try_new(
        TenantMaintenanceRuntimeConfig {
            tick_interval: Duration::from_secs(10),
        },
        runtime_tick,
    )
    .expect("test runtime config should be valid");
    let (report, _) = tokio::join!(runtime.run_until_cancelled(&cancellation), async {
        time::advance(Duration::from_secs(31)).await;
    });

    assert_eq!(report.failed_ticks, 1);
    assert_eq!(report.successful_ticks, 2);
    assert!(matches!(
        report.last_tick,
        Some(RuntimeTickReport::Succeeded(3))
    ));
}

#[tokio::test(start_paused = true)]
async fn already_cancelled_token_does_not_tick() {
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_for_tick = Arc::clone(&hits);
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let runtime_tick = FnRuntimeTick::new(move || {
        let hits_for_tick = Arc::clone(&hits_for_tick);
        async move {
            hits_for_tick.fetch_add(1, Ordering::SeqCst);
            Ok::<(), TestError>(())
        }
    });
    let mut runtime = TenantMaintenanceRuntime::try_new(
        TenantMaintenanceRuntimeConfig {
            tick_interval: Duration::from_secs(10),
        },
        runtime_tick,
    )
    .expect("test runtime config should be valid");

    let report = runtime.run_until_cancelled(&cancellation).await;

    assert_eq!(hits.load(Ordering::SeqCst), 0);
    assert_eq!(report.successful_ticks, 0);
    assert_eq!(report.failed_ticks, 0);
    assert_eq!(report.last_tick, None);
    assert!(report.cancelled);
}

#[tokio::test(start_paused = true)]
async fn failed_last_tick_reports_safe_error_without_stopping() {
    let cancellation = CancellationToken::new();
    let cancellation_for_tick = cancellation.clone();
    let runtime_tick = FnRuntimeTick::new(move || {
        let cancellation_for_tick = cancellation_for_tick.clone();
        async move {
            cancellation_for_tick.cancel();
            Err::<usize, TestError>(TestError("safe_failure"))
        }
    });

    let mut runtime = TenantMaintenanceRuntime::try_new(
        TenantMaintenanceRuntimeConfig {
            tick_interval: Duration::from_secs(10),
        },
        runtime_tick,
    )
    .expect("test runtime config should be valid");
    let (report, _) = tokio::join!(runtime.run_until_cancelled(&cancellation), async {
        time::advance(Duration::from_secs(1)).await;
    });

    assert_eq!(report.failed_ticks, 1);
    assert_eq!(report.successful_ticks, 0);
    assert!(matches!(
        report.last_tick,
        Some(RuntimeTickReport::Failed(RuntimeTickFailure { safe_error }))
            if safe_error == "safe_failure"
    ));
}

#[test]
fn zero_interval_is_rejected() {
    let result = TenantMaintenanceRuntimeConfig {
        tick_interval: Duration::ZERO,
    }
    .validate();
    assert_eq!(
        result,
        Err(TenantMaintenanceRuntimeConfigValidationError::ZeroTickInterval)
    );
}

#[test]
fn registry_rejects_empty_duplicate_or_blank_tenants() {
    let config = TenantMaintenanceRuntimeConfig {
        tick_interval: Duration::from_secs(10),
    };

    let empty =
        TenantMaintenanceRuntimeRegistry::<RegistryTestTick>::try_new(config.clone(), Vec::new());
    assert!(matches!(
        empty,
        Err(TenantMaintenanceRuntimeRegistryValidationError::EmptyRegistry)
    ));

    let blank = TenantMaintenanceRuntimeRegistry::try_new(
        config.clone(),
        vec![RuntimeTenantTick::new(
            " ",
            RegistryTestTick::succeeded(Arc::new(AtomicUsize::new(0)), 1),
        )],
    );
    assert!(matches!(
        blank,
        Err(TenantMaintenanceRuntimeRegistryValidationError::EmptyTenantId)
    ));

    let duplicate = TenantMaintenanceRuntimeRegistry::try_new(
        config,
        vec![
            RuntimeTenantTick::new(
                "tenant_a",
                RegistryTestTick::succeeded(Arc::new(AtomicUsize::new(0)), 1),
            ),
            RuntimeTenantTick::new(
                "tenant_a",
                RegistryTestTick::succeeded(Arc::new(AtomicUsize::new(0)), 2),
            ),
        ],
    );
    assert!(matches!(
        duplicate,
        Err(TenantMaintenanceRuntimeRegistryValidationError::DuplicateTenantId(
            tenant_id
        )) if tenant_id == "tenant_a"
    ));
}

#[tokio::test(start_paused = true)]
async fn registry_runs_multiple_tenants_and_isolates_failures() {
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_for_first = Arc::clone(&calls);
    let calls_for_second = Arc::clone(&calls);
    let cancellation = CancellationToken::new();
    let cancellation_for_second = cancellation.clone();

    let first = RuntimeTenantTick::new(
        "tenant_a",
        RegistryTestTick::failed(calls_for_first, "first_failed"),
    );
    let second = RuntimeTenantTick::new(
        "tenant_b",
        RegistryTestTick::succeeded(calls_for_second, 7).with_cancellation(cancellation_for_second),
    );

    let mut registry = TenantMaintenanceRuntimeRegistry::try_new(
        TenantMaintenanceRuntimeConfig {
            tick_interval: Duration::from_secs(10),
        },
        vec![first, second],
    )
    .expect("registry config should be valid");

    let (report, _) = tokio::join!(registry.run_until_cancelled(&cancellation), async {
        time::advance(Duration::from_secs(1)).await;
    });

    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(report.completed_rounds, 1);
    assert_eq!(report.tenant_reports.len(), 2);
    assert_eq!(report.tenant_reports[0].tenant_id, "tenant_a");
    assert_eq!(report.tenant_reports[0].failed_ticks, 1);
    assert!(matches!(
        &report.tenant_reports[0].last_tick,
        Some(RuntimeTickReport::Failed(RuntimeTickFailure { safe_error }))
            if safe_error == "first_failed"
    ));
    assert_eq!(report.tenant_reports[1].tenant_id, "tenant_b");
    assert_eq!(report.tenant_reports[1].successful_ticks, 1);
    assert!(matches!(
        report.tenant_reports[1].last_tick,
        Some(RuntimeTickReport::Succeeded(7))
    ));
}

#[tokio::test(start_paused = true)]
async fn registry_already_cancelled_token_does_not_tick_any_tenant() {
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_for_tick = Arc::clone(&calls);
    let cancellation = CancellationToken::new();
    cancellation.cancel();

    let mut registry = TenantMaintenanceRuntimeRegistry::try_new(
        TenantMaintenanceRuntimeConfig {
            tick_interval: Duration::from_secs(10),
        },
        vec![RuntimeTenantTick::new(
            "tenant_a",
            RegistryTestTick::succeeded(calls_for_tick, 1),
        )],
    )
    .expect("registry config should be valid");

    let report = registry.run_until_cancelled(&cancellation).await;

    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(report.completed_rounds, 0);
    assert_eq!(report.tenant_reports[0].successful_ticks, 0);
    assert_eq!(report.tenant_reports[0].failed_ticks, 0);
    assert_eq!(report.tenant_reports[0].last_tick, None);
    assert!(report.cancelled);
}

#[tokio::test]
async fn registry_builder_supports_static_discovery_and_canonical_tenant_ids() {
    let config = TenantMaintenanceRuntimeConfig {
        tick_interval: Duration::from_secs(10),
    };
    let builder = TenantMaintenanceRuntimeRegistryBuilder::new(config);
    let mut discovery = StaticRuntimeTenantDiscovery::new(vec![" tenant_a ", "tenant_b"]);
    let mut factory = QueueFactory::new(vec![
        Ok(RegistryTestTick::succeeded(
            Arc::new(AtomicUsize::new(0)),
            1,
        )),
        Ok(RegistryTestTick::succeeded(
            Arc::new(AtomicUsize::new(0)),
            2,
        )),
    ]);

    let mut registry = builder
        .build::<RegistryTestTick, _, _>(&mut discovery, &mut factory)
        .await
        .expect("builder should create registry");
    let report = registry.run_once_round().await;

    assert_eq!(report.completed_rounds, 1);
    assert_eq!(report.tenant_reports.len(), 2);
    assert_eq!(report.tenant_reports[0].tenant_id, "tenant_a");
    assert_eq!(report.tenant_reports[1].tenant_id, "tenant_b");
}

#[tokio::test]
async fn registry_builder_rejects_empty_blank_and_duplicate_tenants() {
    let config = TenantMaintenanceRuntimeConfig {
        tick_interval: Duration::from_secs(10),
    };

    let mut empty_discovery = StaticRuntimeTenantDiscovery::new(Vec::<String>::new());
    let mut factory = QueueFactory::new(Vec::new());
    let empty = TenantMaintenanceRuntimeRegistryBuilder::new(config.clone())
        .build::<RegistryTestTick, _, _>(&mut empty_discovery, &mut factory)
        .await;
    assert!(matches!(
        empty,
        Err(TenantMaintenanceRuntimeRegistryBuildError::EmptyRegistry)
    ));

    let mut blank_discovery = StaticRuntimeTenantDiscovery::new(vec![" "]);
    let blank = TenantMaintenanceRuntimeRegistryBuilder::new(config.clone())
        .build::<RegistryTestTick, _, _>(&mut blank_discovery, &mut factory)
        .await;
    assert!(matches!(
        blank,
        Err(TenantMaintenanceRuntimeRegistryBuildError::EmptyTenantId)
    ));

    let mut duplicate_discovery = StaticRuntimeTenantDiscovery::new(vec!["tenant_a", " tenant_a "]);
    let duplicate = TenantMaintenanceRuntimeRegistryBuilder::new(config)
        .build::<RegistryTestTick, _, _>(&mut duplicate_discovery, &mut factory)
        .await;
    assert!(matches!(
        duplicate,
        Err(TenantMaintenanceRuntimeRegistryBuildError::DuplicateTenantId(tenant_id))
            if tenant_id == "tenant_a"
    ));
}

#[tokio::test]
async fn registry_builder_maps_discovery_error_to_safe_error() {
    let mut discovery = FailingDiscovery;
    let mut factory = QueueFactory::new(Vec::new());
    let result = TenantMaintenanceRuntimeRegistryBuilder::new(TenantMaintenanceRuntimeConfig {
        tick_interval: Duration::from_secs(10),
    })
    .build::<RegistryTestTick, _, _>(&mut discovery, &mut factory)
    .await;

    assert!(matches!(
        result,
        Err(TenantMaintenanceRuntimeRegistryBuildError::DiscoveryFailed { safe_error })
            if safe_error == "tenant_discovery_failed"
    ));
}

#[tokio::test]
async fn registry_builder_maps_factory_error_with_tenant_id_and_safe_error() {
    let mut discovery = StaticRuntimeTenantDiscovery::new(vec!["tenant_a"]);
    let mut factory = QueueFactory::new(vec![Err(FactoryTestError(
        "raw_factory_error_should_not_leak",
    ))]);
    let result = TenantMaintenanceRuntimeRegistryBuilder::new(TenantMaintenanceRuntimeConfig {
        tick_interval: Duration::from_secs(10),
    })
    .build::<RegistryTestTick, _, _>(&mut discovery, &mut factory)
    .await;

    assert!(matches!(
        result,
        Err(TenantMaintenanceRuntimeRegistryBuildError::TickFactoryFailed {
            tenant_id,
            safe_error
        }) if tenant_id == "tenant_a" && safe_error == "tenant_tick_factory_failed"
    ));
}

#[test]
fn postgres_runtime_tenant_discovery_safe_error_does_not_echo_raw_input() {
    let raw = "db password leaked in SQL";
    let safe = PostgresRuntimeTenantDiscovery::map_safe_error(
        &PostgresRepositoryError::UnknownTenantStatus(raw.to_string()),
    );
    assert_eq!(safe, "tenant_discovery_failed: unknown_tenant_status");
    assert!(!safe.contains("password"));
    assert!(!safe.contains("sql"));
}

#[test]
fn postgres_runtime_tenant_discovery_safe_error_maps_typed_errors() {
    let safe = PostgresRuntimeTenantDiscovery::map_safe_error(
        &PostgresRepositoryError::UnknownTenantStatus("active-ish".to_string()),
    );
    assert_eq!(safe, "tenant_discovery_failed: unknown_tenant_status");
}

#[test]
fn postgres_repository_safe_error_reuses_reason_with_context_prefix() {
    let error =
        PostgresRepositoryError::UnknownTenantStatus("raw tenant status with password".to_string());

    assert_eq!(
        postgres_repository_safe_error("tenant_discovery_failed", &error),
        "tenant_discovery_failed: unknown_tenant_status"
    );
    assert_eq!(
        postgres_repository_safe_error("tenant_maintenance_runtime_tick_failed", &error),
        "tenant_maintenance_runtime_tick_failed: unknown_tenant_status"
    );
    assert_eq!(
        postgres_repository_safe_error_reason(&error),
        "unknown_tenant_status"
    );

    let safe = postgres_repository_safe_error("tenant_maintenance_runtime_tick_failed", &error);
    assert!(!safe.contains("password"));
    assert!(!safe.contains("raw tenant status"));
}

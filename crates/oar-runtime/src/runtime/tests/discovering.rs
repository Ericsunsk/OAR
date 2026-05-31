use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::time;
use tokio_util::sync::CancellationToken;

use super::super::{
    DiscoveringRuntimeRoundReport, DiscoveringTenantMaintenanceRuntime, RuntimeTenantDiscovery,
    RuntimeTenantDiscoveryFuture, RuntimeTickFailure, RuntimeTickReport,
};
use super::support::{
    runtime_config, FactoryTestError, FailingDiscovery, QueueFactory, RegistryTestTick,
};

struct QueueDiscovery {
    rounds: VecDeque<Vec<String>>,
}

impl QueueDiscovery {
    fn new(rounds: Vec<Vec<&str>>) -> Self {
        Self {
            rounds: rounds
                .into_iter()
                .map(|round| round.into_iter().map(str::to_string).collect())
                .collect(),
        }
    }
}

impl RuntimeTenantDiscovery for QueueDiscovery {
    type Error = super::support::DiscoveryTestError;

    fn discover_tenant_ids(&mut self) -> RuntimeTenantDiscoveryFuture<'_, Self::Error> {
        let next = self
            .rounds
            .pop_front()
            .expect("test discovery should have enough queued rounds");
        Box::pin(async move { Ok(next) })
    }

    fn safe_error(_error: &Self::Error) -> String {
        "tenant_discovery_failed".to_string()
    }
}

#[tokio::test]
async fn discovering_runtime_discovers_and_builds_ticks_each_round() {
    let calls = Arc::new(AtomicUsize::new(0));
    let first_calls = Arc::clone(&calls);
    let second_calls = Arc::clone(&calls);
    let discovery = QueueDiscovery::new(vec![vec!["tenant_a"], vec!["tenant_b"]]);
    let factory = QueueFactory::new(vec![
        Ok(RegistryTestTick::succeeded(first_calls, 1)),
        Ok(RegistryTestTick::succeeded(second_calls, 2)),
    ]);
    let mut runtime =
        DiscoveringTenantMaintenanceRuntime::try_new(runtime_config(), discovery, factory)
            .expect("runtime config should be valid");

    let first = runtime.run_once_round().await;
    let second = runtime.run_once_round().await;

    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert!(matches!(
        first,
        DiscoveringRuntimeRoundReport::Succeeded(report)
            if report.completed_rounds == 1
                && report.tenant_reports[0].tenant_id == "tenant_a"
                && matches!(
                    report.tenant_reports[0].last_tick,
                    Some(RuntimeTickReport::Succeeded(1))
                )
    ));
    assert!(matches!(
        second,
        DiscoveringRuntimeRoundReport::Succeeded(report)
            if report.completed_rounds == 1
                && report.tenant_reports[0].tenant_id == "tenant_b"
                && matches!(
                    report.tenant_reports[0].last_tick,
                    Some(RuntimeTickReport::Succeeded(2))
                )
    ));
}

#[tokio::test]
async fn discovering_runtime_treats_empty_discovery_as_successful_empty_round() {
    let discovery = QueueDiscovery::new(vec![Vec::new()]);
    let factory = QueueFactory::new(Vec::new());
    let mut runtime =
        DiscoveringTenantMaintenanceRuntime::try_new(runtime_config(), discovery, factory)
            .expect("runtime config should be valid");

    let round = runtime.run_once_round().await;

    assert!(matches!(
        round,
        DiscoveringRuntimeRoundReport::Succeeded(report)
            if report.completed_rounds == 1 && report.tenant_reports.is_empty()
    ));
}

#[tokio::test]
async fn discovering_runtime_reports_discovery_and_factory_errors_as_safe_failures() {
    let mut discovery_failure = DiscoveringTenantMaintenanceRuntime::<
        FailingDiscovery,
        QueueFactory,
        RegistryTestTick,
    >::try_new(
        runtime_config(),
        FailingDiscovery,
        QueueFactory::new(Vec::new()),
    )
    .expect("runtime config should be valid");

    let discovery_round = discovery_failure.run_once_round().await;

    assert!(matches!(
        discovery_round,
        DiscoveringRuntimeRoundReport::Failed(RuntimeTickFailure { safe_error })
            if safe_error == "tenant_discovery_failed"
    ));

    let discovery = QueueDiscovery::new(vec![vec!["tenant_a"]]);
    let factory = QueueFactory::new(vec![Err(FactoryTestError(
        "raw_factory_error_should_not_leak",
    ))]);
    let mut factory_failure =
        DiscoveringTenantMaintenanceRuntime::try_new(runtime_config(), discovery, factory)
            .expect("runtime config should be valid");

    let factory_round = factory_failure.run_once_round().await;

    assert!(matches!(
        factory_round,
        DiscoveringRuntimeRoundReport::Failed(RuntimeTickFailure { safe_error })
            if safe_error == "tenant_tick_factory_failed"
    ));
}

#[tokio::test(start_paused = true)]
async fn discovering_runtime_runs_until_cancelled() {
    let calls = Arc::new(AtomicUsize::new(0));
    let tick_calls = Arc::clone(&calls);
    let cancellation = CancellationToken::new();
    let discovery = QueueDiscovery::new(vec![vec!["tenant_a"]]);
    let factory = QueueFactory::new(vec![Ok(
        RegistryTestTick::succeeded(tick_calls, 7).with_cancellation(cancellation.clone())
    )]);
    let mut runtime =
        DiscoveringTenantMaintenanceRuntime::try_new(runtime_config(), discovery, factory)
            .expect("runtime config should be valid");

    let (report, _) = tokio::join!(runtime.run_until_cancelled(&cancellation), async {
        time::advance(Duration::from_secs(1)).await;
    });

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(report.successful_rounds, 1);
    assert_eq!(report.failed_rounds, 0);
    assert!(report.cancelled);
}

#[tokio::test(start_paused = true)]
async fn discovering_runtime_observer_receives_each_completed_round() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::new(Mutex::new(Vec::new()));
    let observed_rounds = Arc::clone(&observed);
    let tick_calls = Arc::clone(&calls);
    let cancellation = CancellationToken::new();
    let discovery = QueueDiscovery::new(vec![vec!["tenant_a"]]);
    let factory = QueueFactory::new(vec![Ok(
        RegistryTestTick::succeeded(tick_calls, 7).with_cancellation(cancellation.clone())
    )]);
    let mut runtime =
        DiscoveringTenantMaintenanceRuntime::try_new(runtime_config(), discovery, factory)
            .expect("runtime config should be valid");

    let (report, _) = tokio::join!(
        runtime.run_until_cancelled_with_observer(&cancellation, move |round| {
            let status = match round {
                DiscoveringRuntimeRoundReport::Succeeded(_) => "succeeded",
                DiscoveringRuntimeRoundReport::Failed(_) => "failed",
            };
            observed_rounds
                .lock()
                .expect("observed rounds")
                .push(status);
        }),
        async {
            time::advance(Duration::from_secs(1)).await;
        }
    );

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(report.successful_rounds, 1);
    assert_eq!(
        observed.lock().expect("observed rounds").as_slice(),
        ["succeeded"]
    );
}

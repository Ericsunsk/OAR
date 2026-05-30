use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::time;
use tokio_util::sync::CancellationToken;

use super::super::{
    RuntimeTickFailure, RuntimeTickFuture, RuntimeTickReport, TenantMaintenanceRuntime,
    TenantMaintenanceRuntimeConfig, TenantMaintenanceRuntimeConfigValidationError,
};
use super::support::{assert_send, runtime_config, FnRuntimeTick, TestError};

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

    let mut runtime = TenantMaintenanceRuntime::try_new(runtime_config(), runtime_tick)
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
    let mut runtime = TenantMaintenanceRuntime::try_new(runtime_config(), runtime_tick)
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

    let mut runtime = TenantMaintenanceRuntime::try_new(runtime_config(), runtime_tick)
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
    let mut runtime = TenantMaintenanceRuntime::try_new(runtime_config(), runtime_tick)
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

    let mut runtime = TenantMaintenanceRuntime::try_new(runtime_config(), runtime_tick)
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

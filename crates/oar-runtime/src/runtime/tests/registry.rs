use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::time;
use tokio_util::sync::CancellationToken;

use super::super::{
    RuntimeTenantTick, RuntimeTickFailure, RuntimeTickReport, TenantMaintenanceRuntimeRegistry,
    TenantMaintenanceRuntimeRegistryValidationError,
};
use super::support::{runtime_config, RegistryTestTick};

#[test]
fn registry_rejects_empty_duplicate_or_blank_tenants() {
    let config = runtime_config();

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

    let mut registry =
        TenantMaintenanceRuntimeRegistry::try_new(runtime_config(), vec![first, second])
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
        runtime_config(),
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

use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

use super::super::{
    StaticRuntimeTenantDiscovery, TenantMaintenanceRuntimeRegistryBuildError,
    TenantMaintenanceRuntimeRegistryBuilder,
};
use super::support::{
    runtime_config, FactoryTestError, FailingDiscovery, QueueFactory, RegistryTestTick,
};

#[tokio::test]
async fn registry_builder_supports_static_discovery_and_canonical_tenant_ids() {
    let builder = TenantMaintenanceRuntimeRegistryBuilder::new(runtime_config());
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
    let config = runtime_config();

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
    let result = TenantMaintenanceRuntimeRegistryBuilder::new(runtime_config())
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
    let result = TenantMaintenanceRuntimeRegistryBuilder::new(runtime_config())
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

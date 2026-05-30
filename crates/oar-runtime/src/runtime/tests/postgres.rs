use oar_core::storage::postgres::PostgresRepositoryError;

use super::super::postgres::{
    postgres_repository_safe_error, postgres_repository_safe_error_reason,
};
use super::super::PostgresRuntimeTenantDiscovery;

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

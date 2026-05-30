use super::super::PostgresRuntimeTenantDiscovery;
use oar_core::storage::postgres::PostgresRepositoryError;

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

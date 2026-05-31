use hyper::http::StatusCode;
use serde_json::json;

use crate::response::{json_facade_response, FacadeResponse};
use crate::runtime::OarHttpFacadeRuntime;
use crate::tenant_maintenance_daemon_status::TenantMaintenanceDaemonStatusSnapshot;

pub(crate) fn healthz_response(runtime: Option<&OarHttpFacadeRuntime>) -> FacadeResponse {
    let tenant_maintenance = runtime
        .map(|runtime| runtime.tenant_maintenance_daemon_status().snapshot())
        .unwrap_or_else(TenantMaintenanceDaemonStatusSnapshot::disabled);
    let body = json!({
        "status": "ok",
        "service": "oar-http-facade",
        "tenant_maintenance": tenant_maintenance_health_value(&tenant_maintenance),
    });
    json_facade_response(StatusCode::OK, body)
}

fn tenant_maintenance_health_value(
    snapshot: &TenantMaintenanceDaemonStatusSnapshot,
) -> serde_json::Value {
    json!({
        "enabled": snapshot.enabled,
        "daemon_state": snapshot.state,
        "successful_rounds": snapshot.successful_rounds,
        "failed_rounds": snapshot.failed_rounds,
        "failed_tenant_ticks": snapshot.failed_tenant_ticks,
        "last_round_status": snapshot.last_round_status,
        "last_round_tenant_count": snapshot.last_round_tenant_count,
        "last_round_failed_tenant_count": snapshot.last_round_failed_tenant_count,
        "last_failure_code": snapshot.last_failure_code,
    })
}

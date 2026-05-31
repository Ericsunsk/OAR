use std::sync::Arc;

use hyper::http::{Method, StatusCode};
use serde_json::Value;

use crate::tenant_maintenance_daemon_failure::{
    classify_failure_code, TenantMaintenanceDaemonFailureCode,
};
use crate::tenant_maintenance_daemon_status::TenantMaintenanceDaemonStatusHandle;
use crate::{dispatch_request, dispatch_request_with_runtime, OarHttpFacadeRuntime};

#[test]
fn healthz_returns_safe_service_status() {
    let response = dispatch_request(&Method::GET, "/healthz", None, None);
    let body: Value = serde_json::from_str(&response.body).expect("json");

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert_eq!(body["tenant_maintenance"]["enabled"], false);
    assert_eq!(body["tenant_maintenance"]["daemon_state"], "disabled");
    assert!(!response.body.contains("token"));
}

#[tokio::test]
async fn healthz_with_runtime_reports_safe_tenant_maintenance_status() {
    let response = dispatch_request_with_runtime(
        Arc::new(OarHttpFacadeRuntime::disabled()),
        &Method::GET,
        "/healthz",
        None,
        None,
        None,
    )
    .await;
    let body: Value = serde_json::from_str(&response.body).expect("json");
    let tenant = &body["tenant_maintenance"];

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(tenant["enabled"], false);
    assert_eq!(tenant["daemon_state"], "disabled");
    assert_eq!(tenant["successful_rounds"], 0);
    assert_eq!(tenant["failed_rounds"], 0);
    assert_eq!(tenant["failed_tenant_ticks"], 0);
    assert_eq!(tenant["daemon_failures"], 0);
    assert_eq!(tenant["last_round_status"], Value::Null);
    assert_eq!(tenant["last_round_tenant_count"], 0);
    assert_eq!(tenant["last_round_failed_tenant_count"], 0);
    assert_eq!(tenant["last_failure_code"], Value::Null);
    assert_eq!(tenant["last_daemon_failure_code"], Value::Null);
    assert_eq!(
        tenant["stages"]["scheduled_sweep"]["last_status"],
        Value::Null
    );
    assert_eq!(tenant["stages"]["outbox_drain"]["last_status"], Value::Null);
    assert!(!response.body.contains("token"));
    assert!(!response.body.contains("secret"));
}

#[tokio::test]
async fn healthz_does_not_echo_unclassified_failure_details() {
    let status = TenantMaintenanceDaemonStatusHandle::for_enabled(true);
    status.mark_daemon_failed(classify_failure_code(
        "https://tenant.example.test/callback?code=auth-code Bearer X-Api-Key tenant_secret_id",
    ));
    let runtime = OarHttpFacadeRuntime {
        tenant_maintenance_daemon_status: status,
        ..OarHttpFacadeRuntime::disabled()
    };

    let response = dispatch_request_with_runtime(
        Arc::new(runtime),
        &Method::GET,
        "/healthz",
        None,
        None,
        None,
    )
    .await;
    let body: Value = serde_json::from_str(&response.body).expect("json");
    let tenant = &body["tenant_maintenance"];

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(tenant["daemon_state"], "failed");
    assert_eq!(
        tenant["last_daemon_failure_code"],
        "tenant_maintenance_failure"
    );
    assert!(!response.body.contains("tenant.example.test"));
    assert!(!response.body.contains("auth-code"));
    assert!(!response.body.contains("Bearer"));
    assert!(!response.body.contains("X-Api-Key"));
    assert!(!response.body.contains("tenant_secret_id"));
}

#[test]
fn readyz_reports_ok_for_disabled_tenant_maintenance() {
    let response = dispatch_request(&Method::GET, "/readyz", None, None);
    let body: Value = serde_json::from_str(&response.body).expect("json");

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert_eq!(body["reason_code"], "tenant_maintenance_disabled");
    assert_eq!(body["tenant_maintenance"]["daemon_state"], "disabled");
}

#[tokio::test]
async fn readyz_reports_unavailable_for_failed_tenant_maintenance_daemon() {
    let status = TenantMaintenanceDaemonStatusHandle::for_enabled(true);
    status.mark_daemon_failed(TenantMaintenanceDaemonFailureCode::DaemonTaskFailed);
    let runtime = OarHttpFacadeRuntime {
        tenant_maintenance_daemon_status: status,
        ..OarHttpFacadeRuntime::disabled()
    };

    let response =
        dispatch_request_with_runtime(Arc::new(runtime), &Method::GET, "/readyz", None, None, None)
            .await;
    let body: Value = serde_json::from_str(&response.body).expect("json");

    assert_eq!(response.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["status"], "not_ready");
    assert_eq!(body["reason_code"], "tenant_maintenance_daemon_failed");
    assert_eq!(
        body["tenant_maintenance"]["last_daemon_failure_code"],
        "tenant_maintenance_daemon_task_failed"
    );
}

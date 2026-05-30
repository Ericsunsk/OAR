use hyper::http::{Method, StatusCode};
use serde_json::Value;

use crate::dispatch_request;

#[test]
fn healthz_returns_safe_service_status() {
    let response = dispatch_request(&Method::GET, "/healthz", None, None);
    let body: Value = serde_json::from_str(&response.body).expect("json");

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert!(!response.body.contains("token"));
}

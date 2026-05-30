use hyper::http::{Method, StatusCode};
use serde_json::Value;

use crate::dispatch_request;

#[test]
fn snapshot_requires_verified_oar_session_store() {
    let unauthorized = dispatch_request(&Method::GET, "/review-inbox/snapshot", None, None);
    assert_eq!(unauthorized.status, StatusCode::UNAUTHORIZED);

    let response = dispatch_request(
        &Method::GET,
        "/review-inbox/snapshot",
        Some("Bearer oar_session_dev"),
        Some("application/json"),
    );
    let body: Value = serde_json::from_str(&response.body).expect("json");

    assert_eq!(response.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"], "oar_session_verification_unavailable");
}

#[test]
fn decisions_require_verified_oar_session_store() {
    let response = dispatch_request(
        &Method::POST,
        "/review-inbox/decisions",
        Some("Bearer oar_session_dev"),
        Some("application/json"),
    );
    let body: Value = serde_json::from_str(&response.body).expect("json");

    assert_eq!(response.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"], "oar_session_verification_unavailable");
}

#[test]
fn agent_stream_requires_verified_oar_session_store() {
    let unauthorized = dispatch_request(&Method::POST, "/agent/stream", None, None);
    assert_eq!(unauthorized.status, StatusCode::UNAUTHORIZED);

    let response = dispatch_request(
        &Method::POST,
        "/agent/stream",
        Some("Bearer oar_session_dev"),
        Some("text/event-stream"),
    );
    let body: Value = serde_json::from_str(&response.body).expect("json");

    assert_eq!(response.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"], "oar_session_verification_unavailable");
}

#[test]
fn agent_settings_routes_require_verified_oar_session_store() {
    let unauthorized = dispatch_request(&Method::GET, "/agent/settings", None, None);
    assert_eq!(unauthorized.status, StatusCode::UNAUTHORIZED);

    for (method, path) in [
        (&Method::GET, "/agent/settings"),
        (&Method::PUT, "/agent/settings"),
        (&Method::DELETE, "/agent/settings"),
        (&Method::POST, "/agent/model-catalog/preview"),
    ] {
        let response = dispatch_request(
            method,
            path,
            Some("Bearer oar_session_dev"),
            Some("application/json"),
        );
        let body: Value = serde_json::from_str(&response.body).expect("json");

        assert_eq!(response.status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["error"], "oar_session_verification_unavailable");
    }
}

#[test]
fn auth_routes_do_not_fake_real_feishu_login() {
    let create = dispatch_request(&Method::POST, "/auth/feishu/qr-sessions", None, None);
    let poll = dispatch_request(&Method::GET, "/auth/feishu/qr-sessions/qr_dev", None, None);
    let events = dispatch_request(
        &Method::GET,
        "/auth/feishu/qr-sessions/qr_dev/events",
        None,
        Some("text/event-stream"),
    );

    assert_eq!(create.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(poll.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(events.status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(!create.body.contains("access_token"));
    assert!(!poll.body.contains("refresh_token"));
    assert!(!events.body.contains("authorization"));
}

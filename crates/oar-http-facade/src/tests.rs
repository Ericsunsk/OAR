use super::*;
use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};

use hyper::http::{Method, StatusCode};
use oar_core::domain::device_sync::{DeviceEntryPoint, SessionState};
use oar_core::storage::postgres::StoredDeviceSession;
use serde_json::Value;

mod feishu_auth;

#[test]
fn healthz_returns_safe_service_status() {
    let response = dispatch_request(&Method::GET, "/healthz", None, None);
    let body: Value = serde_json::from_str(&response.body).expect("json");

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert!(!response.body.contains("token"));
}

#[test]
fn config_defaults_to_localhost_and_accepts_docker_bind_override() {
    let default_config = OarHttpFacadeConfig::from_env_map(&|_| None).expect("default config");
    let docker_config = OarHttpFacadeConfig::from_env_map(&|key| {
        (key == "OAR_HTTP_BIND_ADDR").then(|| "0.0.0.0:8080".to_string())
    })
    .expect("docker config");

    assert_eq!(
        default_config.bind_addr,
        "127.0.0.1:8080".parse::<SocketAddr>().expect("addr")
    );
    assert_eq!(
        docker_config.bind_addr,
        "0.0.0.0:8080".parse::<SocketAddr>().expect("addr")
    );
}

#[test]
fn config_rejects_invalid_bind_override_without_echoing_in_display() {
    let error = OarHttpFacadeConfig::from_env_map(&|key| {
        (key == "OAR_HTTP_BIND_ADDR").then(|| "not an address".to_string())
    })
    .expect_err("invalid config");

    assert_eq!(
        error.to_string(),
        "oar_http_facade_config_invalid: invalid_bind_addr"
    );
    assert!(!error.to_string().contains("not an address"));
}

#[test]
fn runtime_disables_auth_when_env_absent_and_rejects_partial_auth_config() {
    let disabled = OarHttpFacadeRuntime::from_env_map(&|_| None).expect("disabled runtime");
    assert!(disabled.feishu_login.is_none());
    assert!(disabled.agent.is_none());

    let partial = OarHttpFacadeRuntime::from_env_map(&|key| {
        (key == "OAR_FEISHU_APP_ID").then(|| "cli_test".to_string())
    })
    .expect_err("partial auth config");

    assert_eq!(
        partial.to_string(),
        "oar_feishu_auth_config_partial".to_string()
    );
    assert!(!format!("{partial:?}").contains("cli_test"));
}

#[test]
fn runtime_accepts_agent_config_without_leaking_secret() {
    let runtime = OarHttpFacadeRuntime::from_env_map(&|key| match key {
        "OAR_AGENT_OPENAI_BASE_URL" => Some("https://llm.example.test/v1".to_string()),
        "OAR_AGENT_OPENAI_API_KEY" => Some("sk-sensitive".to_string()),
        "OAR_AGENT_OPENAI_MODEL" => Some("agent-model".to_string()),
        _ => None,
    })
    .expect("runtime");

    assert!(runtime.feishu_login.is_none());
    assert!(runtime.agent.is_some());
    assert!(!format!("{runtime:?}").contains("sk-sensitive"));
}

#[test]
fn runtime_accepts_anthropic_agent_config_without_leaking_secret() {
    let runtime = OarHttpFacadeRuntime::from_env_map(&|key| match key {
        "OAR_AGENT_PROVIDER" => Some("anthropic".to_string()),
        "OAR_AGENT_ANTHROPIC_API_KEY" => Some("sk-ant-sensitive".to_string()),
        "OAR_AGENT_ANTHROPIC_MODEL" => Some("claude-sonnet-test".to_string()),
        _ => None,
    })
    .expect("runtime");

    assert!(runtime.feishu_login.is_none());
    assert!(runtime.agent.is_some());
    assert!(!format!("{runtime:?}").contains("sk-ant-sensitive"));
}

#[test]
fn runtime_rejects_partial_agent_config_without_leaking_secret() {
    let error = OarHttpFacadeRuntime::from_env_map(&|key| {
        (key == "OAR_AGENT_OPENAI_API_KEY").then(|| "sk-sensitive".to_string())
    })
    .expect_err("partial agent config");

    assert_eq!(error.to_string(), "oar_agent_config_partial");
    assert!(!format!("{error:?}").contains("sk-sensitive"));
}

#[test]
fn bearer_session_id_requires_oar_session_prefix() {
    assert_eq!(
        bearer_session_id(Some("Bearer oar_session_abc")).expect("session"),
        "oar_session_abc"
    );
    assert_eq!(
        bearer_session_id(Some("Bearer other_token")).expect_err("invalid"),
        OarSessionAuthError::InvalidSession
    );
    assert_eq!(
        bearer_session_id(None).expect_err("missing"),
        OarSessionAuthError::MissingBearer
    );
}

#[test]
fn authenticated_context_requires_active_device_session() {
    let active = stored_device_session(SessionState::Active, None, None);
    let context = authenticated_context_from_session(&active).expect("active context");

    assert_eq!(context.session_id, "oar_session_test");
    assert_eq!(context.tenant_id, "tenant_1");
    assert_eq!(context.user_id, "user_1");

    let revoked = stored_device_session(SessionState::Revoked, Some(UNIX_EPOCH), None);
    assert_eq!(
        authenticated_context_from_session(&revoked).expect_err("revoked"),
        OarSessionAuthError::InvalidSession
    );

    let expired = stored_device_session(SessionState::Expired, None, Some(UNIX_EPOCH));
    assert_eq!(
        authenticated_context_from_session(&expired).expect_err("expired"),
        OarSessionAuthError::InvalidSession
    );
}

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

fn stored_device_session(
    state: SessionState,
    revoked_at: Option<SystemTime>,
    expired_at: Option<SystemTime>,
) -> StoredDeviceSession {
    StoredDeviceSession {
        id: "oar_session_test".to_string(),
        tenant_id: "tenant_1".to_string(),
        user_id: "user_1".to_string(),
        entry_point: DeviceEntryPoint::MacOs,
        state,
        sync_stream: "review_inbox".to_string(),
        sync_cursor_value: 0,
        sync_cursor_updated_at: UNIX_EPOCH,
        session_identity_hash: "hash".to_string(),
        last_seen_at: UNIX_EPOCH,
        revoked_at,
        expired_at,
    }
}

use super::*;
use crate::feishu_auth::{
    authorize_test_session, build_feishu_login_persistence_plan, iso8601_utc,
    FeishuLoginPersistenceError,
};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use http_body_util::BodyExt;
use hyper::http::{Method, StatusCode};
use oar_core::domain::device_sync::{DeviceEntryPoint, SessionState};
use oar_core::domain::identity::{ScopeBoundary, TokenGrantState};
use oar_core::storage::postgres::StoredDeviceSession;
use oar_lark_adapter::{
    FeishuOAuthLogin, FeishuOAuthLoginToken, FeishuOAuthLoginUser, SecretString,
};
use serde_json::Value;
use tokio::time;

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
fn configured_runtime_creates_pending_feishu_login_session_with_default_agent_scopes_without_leaking_secret(
) {
    let runtime = OarHttpFacadeRuntime::from_env_map(&|key| match key {
        "OAR_FEISHU_APP_ID" => Some("cli_test".to_string()),
        "OAR_FEISHU_APP_SECRET" => Some("super-secret".to_string()),
        "OAR_FEISHU_REDIRECT_URI" => {
            Some("https://oar.example.test/auth/feishu/callback".to_string())
        }
        _ => None,
    })
    .expect("runtime");

    let response = create_feishu_login_session(runtime.feishu_login.as_deref());
    let body: Value = serde_json::from_str(&response.body).expect("json");

    assert_eq!(response.status, StatusCode::CREATED);
    assert!(body["session_id"].as_str().expect("session id").len() >= 32);
    assert!(body["qr_page_url"]
        .as_str()
        .expect("qr url")
        .contains("/open-apis/authen/v1/authorize"));
    assert!(body["qr_page_url"]
        .as_str()
        .expect("qr url")
        .contains("client_id=cli_test"));
    let qr_page_url = body["qr_page_url"].as_str().expect("qr url");
    for encoded_scope in [
        "scope=offline_access",
        "okr%3Aokr.period%3Areadonly",
        "okr%3Aokr.content%3Areadonly",
        "okr%3Aokr.progress%3Areadonly",
        "okr%3Aokr.progress%3Awriteonly",
        "okr%3Aokr.review%3Areadonly",
        "okr%3Aokr.setting%3Aread",
        "calendar%3Acalendar.free_busy%3Aread",
        "task%3Atask%3Aread",
        "task%3Atask%3Awriteonly",
    ] {
        assert!(
            qr_page_url.contains(encoded_scope),
            "missing encoded scope {encoded_scope} in {qr_page_url}"
        );
    }
    assert!(!body["qr_page_url"]
        .as_str()
        .expect("qr url")
        .contains("auth%3Auser.id%3Aread"));
    assert!(!body["qr_page_url"]
        .as_str()
        .expect("qr url")
        .contains("okr%3Aokr%3A"));
    assert!(!body["qr_page_url"]
        .as_str()
        .expect("qr url")
        .contains("delete"));
    assert!(!response.body.contains("super-secret"));
}

#[test]
fn configured_runtime_uses_explicit_okr_read_scope_for_live_agent_authorization() {
    let runtime = OarHttpFacadeRuntime::from_env_map(&|key| match key {
        "OAR_FEISHU_APP_ID" => Some("cli_test".to_string()),
        "OAR_FEISHU_APP_SECRET" => Some("super-secret".to_string()),
        "OAR_FEISHU_REDIRECT_URI" => {
            Some("https://oar.example.test/auth/feishu/callback".to_string())
        }
        "OAR_FEISHU_AUTH_SCOPE" => Some(
            "offline_access okr:okr.period:readonly okr:okr.content:readonly okr:okr.progress:readonly"
                .to_string(),
        ),
        _ => None,
    })
    .expect("runtime");

    let response = create_feishu_login_session(runtime.feishu_login.as_deref());
    let body: Value = serde_json::from_str(&response.body).expect("json");
    let qr_page_url = body["qr_page_url"].as_str().expect("qr url");

    assert_eq!(response.status, StatusCode::CREATED);
    assert!(qr_page_url.contains("scope=offline_access%20okr%3Aokr.period%3Areadonly%20okr%3Aokr.content%3Areadonly%20okr%3Aokr.progress%3Areadonly"));
    assert!(!qr_page_url.contains("writeonly"));
    assert!(!response.body.contains("super-secret"));
}

#[tokio::test]
async fn async_runtime_requires_grant_key_config_when_database_is_enabled() {
    let error = OarHttpFacadeRuntime::from_env_map_async(&|key| match key {
        "DATABASE_URL" => Some("postgres://oar:oar@127.0.0.1:5432/oar".to_string()),
        "OAR_FEISHU_APP_ID" => Some("cli_test".to_string()),
        "OAR_FEISHU_APP_SECRET" => Some("super-secret".to_string()),
        "OAR_FEISHU_REDIRECT_URI" => {
            Some("https://oar.example.test/auth/feishu/callback".to_string())
        }
        _ => None,
    })
    .await
    .expect_err("database-backed login requires grant encryption key config");

    assert_eq!(error.to_string(), "oar_feishu_grant_config_invalid");
    assert!(!format!("{error:?}").contains("super-secret"));
}

#[test]
fn feishu_login_persistence_plan_builds_stable_redacted_grant() {
    let login = sample_feishu_login(Some("refresh-token-sensitive"));
    let plan = build_feishu_login_persistence_plan(
        &login,
        "oar_session_abc",
        "key-prod-v1",
        [7; 32],
        UNIX_EPOCH + Duration::from_secs(1),
    )
    .expect("plan");

    assert_eq!(plan.tenant.id.0, "feishu_tenant_tenant_1");
    assert_eq!(plan.user.id.0, "feishu_user_tenant_1_ou_123");
    assert_eq!(plan.identity.actor_external_id, "ou_123");
    assert_eq!(plan.grant.identity_id, plan.identity.id.0);
    assert_eq!(plan.grant.scope_boundary, ScopeBoundary::User);
    assert_eq!(
        plan.grant.scopes,
        vec!["auth:user.id:read", "offline_access"]
    );
    assert_eq!(plan.grant.state, TokenGrantState::Valid);
    assert_eq!(plan.grant.issued_at_ms, 1_000);
    assert!(plan.grant.refreshed_at_ms.is_some());
    assert!(plan.grant.expires_at_ms.is_some());
    assert!(plan.grant.encrypted_oauth_grant.len() > "access-token-sensitive".len());
    assert_eq!(plan.session.id.0, "oar_session_abc");
    assert_eq!(plan.session_identity_hash.len(), 64);

    let grant_debug = format!("{:?}", plan.grant);
    assert!(!grant_debug.contains("access-token-sensitive"));
    assert!(!grant_debug.contains("refresh-token-sensitive"));
    assert!(!grant_debug.contains("key-prod-v1"));
    assert!(!grant_debug.contains(&plan.grant.oauth_grant_fingerprint));
    assert!(!contains_bytes(
        &plan.grant.encrypted_oauth_grant,
        b"access-token-sensitive"
    ));
    assert!(!contains_bytes(
        &plan.grant.encrypted_oauth_grant,
        b"refresh-token-sensitive"
    ));
}

#[test]
fn feishu_login_persistence_plan_requires_refresh_token() {
    let login = sample_feishu_login(None);
    let error = build_feishu_login_persistence_plan(
        &login,
        "oar_session_abc",
        "key-prod-v1",
        [7; 32],
        UNIX_EPOCH,
    )
    .expect_err("refresh token required");

    assert_eq!(error, FeishuLoginPersistenceError::MissingRefreshToken);
}

#[tokio::test]
async fn configured_runtime_dispatch_creates_and_polls_pending_feishu_login_session() {
    let runtime = Arc::new(
        OarHttpFacadeRuntime::from_env_map(&|key| match key {
            "OAR_FEISHU_APP_ID" => Some("cli_test".to_string()),
            "OAR_FEISHU_APP_SECRET" => Some("super-secret".to_string()),
            "OAR_FEISHU_REDIRECT_URI" => {
                Some("https://oar.example.test/auth/feishu/callback".to_string())
            }
            _ => None,
        })
        .expect("runtime"),
    );

    let create = dispatch_request_with_runtime(
        Arc::clone(&runtime),
        &Method::POST,
        "/auth/feishu/qr-sessions",
        None,
        None,
        None,
    )
    .await;
    let created: Value = serde_json::from_str(&create.body).expect("create json");
    let session_path = format!(
        "/auth/feishu/qr-sessions/{}",
        created["session_id"].as_str().expect("session id")
    );
    let poll =
        dispatch_request_with_runtime(runtime, &Method::GET, &session_path, None, None, None).await;
    let status: Value = serde_json::from_str(&poll.body).expect("poll json");

    assert_eq!(create.status, StatusCode::CREATED);
    assert_eq!(poll.status, StatusCode::OK);
    assert_eq!(status["status"], "pending");
    assert_eq!(status["qr_session"]["session_id"], created["session_id"]);
    assert!(!create.body.contains("super-secret"));
    assert!(!poll.body.contains("super-secret"));
}

#[tokio::test]
async fn hyper_sse_stream_pushes_authorized_event_when_session_changes() {
    let runtime = Arc::new(
        OarHttpFacadeRuntime::from_env_map(&|key| match key {
            "OAR_FEISHU_APP_ID" => Some("cli_test".to_string()),
            "OAR_FEISHU_APP_SECRET" => Some("super-secret".to_string()),
            "OAR_FEISHU_REDIRECT_URI" => {
                Some("https://oar.example.test/auth/feishu/callback".to_string())
            }
            _ => None,
        })
        .expect("runtime"),
    );

    let create = create_feishu_login_session(runtime.feishu_login.as_deref());
    let created: Value = serde_json::from_str(&create.body).expect("create json");
    let session_id = created["session_id"].as_str().expect("session id");
    let response = feishu_login_session_event_stream_response(
        runtime.feishu_login.clone(),
        session_id.to_string(),
    );

    let login_runtime = runtime.feishu_login.as_ref().expect("feishu login runtime");
    authorize_test_session(login_runtime, session_id);

    let collected = time::timeout(Duration::from_secs(1), response.into_body().collect())
        .await
        .expect("stream should complete")
        .expect("body should collect");
    let body = String::from_utf8(collected.to_bytes().to_vec()).expect("utf8 body");

    assert_eq!(create.status, StatusCode::CREATED);
    assert!(body.contains("event: pending"));
    assert!(body.contains("event: authorized"));
    assert!(body.contains("\"session_id\":\"mock-oar-session\""));
    assert!(!body.contains("super-secret"));
}

#[tokio::test]
async fn callback_without_code_does_not_invalidate_pending_login_session() {
    let runtime = Arc::new(
        OarHttpFacadeRuntime::from_env_map(&|key| match key {
            "OAR_FEISHU_APP_ID" => Some("cli_test".to_string()),
            "OAR_FEISHU_APP_SECRET" => Some("super-secret".to_string()),
            "OAR_FEISHU_REDIRECT_URI" => {
                Some("https://oar.example.test/auth/feishu/callback".to_string())
            }
            _ => None,
        })
        .expect("runtime"),
    );

    let create = dispatch_request_with_runtime(
        Arc::clone(&runtime),
        &Method::POST,
        "/auth/feishu/qr-sessions",
        None,
        None,
        None,
    )
    .await;
    let created: Value = serde_json::from_str(&create.body).expect("create json");
    let session_id = created["session_id"].as_str().expect("session id");

    let callback = dispatch_request_with_runtime(
        Arc::clone(&runtime),
        &Method::GET,
        "/auth/feishu/callback",
        Some(&format!("state={session_id}")),
        None,
        None,
    )
    .await;
    let poll = dispatch_request_with_runtime(
        runtime,
        &Method::GET,
        &format!("/auth/feishu/qr-sessions/{session_id}"),
        None,
        None,
        None,
    )
    .await;
    let status: Value = serde_json::from_str(&poll.body).expect("poll json");

    assert_eq!(callback.status, StatusCode::BAD_REQUEST);
    assert_eq!(poll.status, StatusCode::OK);
    assert_eq!(status["status"], "pending");
    assert_eq!(status["safe_message"], Value::Null);
    assert!(!callback.body.contains("super-secret"));
    assert!(!poll.body.contains("super-secret"));
}

#[test]
fn iso8601_formatter_uses_utc_epoch_contract() {
    assert_eq!(iso8601_utc(UNIX_EPOCH), "1970-01-01T00:00:00Z");
    assert_eq!(
        iso8601_utc(UNIX_EPOCH + Duration::from_secs(86_400)),
        "1970-01-02T00:00:00Z"
    );
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

fn sample_feishu_login(refresh_token: Option<&str>) -> FeishuOAuthLogin {
    FeishuOAuthLogin {
        token: FeishuOAuthLoginToken {
            access_token: SecretString::new("access-token-sensitive"),
            refresh_token: refresh_token.map(SecretString::new),
            expires_in_seconds: 7_200,
            refresh_token_expires_in_seconds: Some(30 * 86_400),
            token_type: Some("Bearer".to_string()),
            scope: Some("offline_access auth:user.id:read offline_access".to_string()),
        },
        user: FeishuOAuthLoginUser {
            open_id: "ou_123".to_string(),
            union_id: Some("on_123".to_string()),
            tenant_key: Some("tenant_1".to_string()),
            display_name: "Alice".to_string(),
        },
    }
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

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

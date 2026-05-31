use crate::{dispatch_request_with_runtime, OarHttpFacadeRuntime};
use hyper::http::{Method, StatusCode};
use serde_json::Value;
use std::sync::Arc;

mod logout;

#[tokio::test]
async fn configured_runtime_dispatch_creates_and_polls_pending_feishu_login_session() {
    let runtime = configured_runtime();

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
async fn callback_without_code_does_not_invalidate_pending_login_session() {
    let runtime = configured_runtime();

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

pub(super) fn configured_runtime() -> Arc<OarHttpFacadeRuntime> {
    Arc::new(OarHttpFacadeRuntime::from_env_map(&configured_env).expect("runtime"))
}

pub(super) fn configured_env(key: &str) -> Option<String> {
    match key {
        "OAR_FEISHU_APP_ID" => Some("cli_test".to_string()),
        "OAR_FEISHU_APP_SECRET" => Some("super-secret".to_string()),
        "OAR_FEISHU_REDIRECT_URI" => {
            Some("https://oar.example.test/auth/feishu/callback".to_string())
        }
        _ => None,
    }
}

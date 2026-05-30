use super::super::postgres_support::{device_session, run_live_postgres_test, seed_user};
use crate::feishu_auth::FeishuLoginRuntime;
use crate::persistence::FacadePersistenceRuntime;
use crate::{dispatch_request_with_runtime, OarHttpFacadeRuntime};
use hyper::http::{Method, StatusCode};
use oar_core::domain::device_sync::SessionState;
use oar_core::storage::postgres::PostgresDeviceSessionRepository;
use serde_json::Value;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

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

#[tokio::test]
async fn configured_runtime_logout_requires_oar_bearer_and_session_store() {
    let runtime = configured_runtime();

    let missing = dispatch_request_with_runtime(
        Arc::clone(&runtime),
        &Method::POST,
        "/auth/logout",
        None,
        None,
        None,
    )
    .await;
    let invalid = dispatch_request_with_runtime(
        Arc::clone(&runtime),
        &Method::POST,
        "/auth/logout",
        None,
        Some("Bearer feishu_token"),
        None,
    )
    .await;
    let unavailable = dispatch_request_with_runtime(
        runtime,
        &Method::POST,
        "/auth/logout",
        None,
        Some("Bearer oar_session_dev"),
        None,
    )
    .await;
    let unavailable_body: Value = serde_json::from_str(&unavailable.body).expect("json");

    assert_eq!(missing.status, StatusCode::UNAUTHORIZED);
    assert_eq!(invalid.status, StatusCode::UNAUTHORIZED);
    assert_eq!(unavailable.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        unavailable_body["error"],
        "oar_session_verification_unavailable"
    );
    assert!(!unavailable.body.contains("super-secret"));
    assert!(!unavailable.body.contains("oar_session_dev"));
}

#[tokio::test]
async fn logout_route_revokes_active_oar_session_and_is_idempotent_when_database_is_available() {
    run_live_postgres_test(
        "logout_route_revokes_active_oar_session",
        |pool| async move {
            let tenant_id = "tenant_logout_route";
            let user_id = "user_logout_route";
            let session_id = "oar_session_logout_route";
            seed_user(&pool, tenant_id, user_id).await?;

            let repository = PostgresDeviceSessionRepository::new(pool.clone());
            let now = SystemTime::UNIX_EPOCH + Duration::from_millis(1_748_310_000_000);
            let session = device_session(tenant_id, user_id, session_id, "review_inbox", 0, now);
            repository
                .upsert_with_identity_hash(&session, "sha256:logout-route")
                .await?;

            let runtime = configured_runtime_with_persistence(pool);
            let first = dispatch_request_with_runtime(
                Arc::clone(&runtime),
                &Method::POST,
                "/auth/logout",
                None,
                Some("Bearer oar_session_logout_route"),
                None,
            )
            .await;
            let first_body: Value = serde_json::from_str(&first.body).expect("first logout json");
            assert_eq!(first.status, StatusCode::OK);
            assert_eq!(first_body["status"], "signed_out");

            let revoked = repository
                .get_by_id(tenant_id, session_id)
                .await?
                .expect("session should still exist after logout");
            assert_eq!(revoked.state, SessionState::Revoked);
            let revoked_at = revoked.revoked_at.expect("revoked_at should be set");

            let second = dispatch_request_with_runtime(
                runtime,
                &Method::POST,
                "/auth/logout",
                None,
                Some("Bearer oar_session_logout_route"),
                None,
            )
            .await;
            let second_body: Value =
                serde_json::from_str(&second.body).expect("second logout json");
            assert_eq!(second.status, StatusCode::OK);
            assert_eq!(second_body["status"], "signed_out");

            let after_second = repository
                .get_by_id(tenant_id, session_id)
                .await?
                .expect("session should still exist after second logout");
            assert_eq!(after_second.state, SessionState::Revoked);
            assert_eq!(after_second.revoked_at, Some(revoked_at));

            Ok(())
        },
    )
    .await;
}

fn configured_runtime() -> Arc<OarHttpFacadeRuntime> {
    Arc::new(OarHttpFacadeRuntime::from_env_map(&configured_env).expect("runtime"))
}

fn configured_runtime_with_persistence(pool: sqlx::PgPool) -> Arc<OarHttpFacadeRuntime> {
    let persistence = FacadePersistenceRuntime::new(pool, "key-test-v1".to_string(), [7; 32]);
    let feishu_login = FeishuLoginRuntime::from_env_map(&configured_env)
        .expect("login runtime")
        .expect("configured login runtime");
    Arc::new(OarHttpFacadeRuntime {
        persistence: Some(persistence),
        feishu_login: Some(Arc::new(feishu_login)),
        agent: None,
        agent_settings: None,
    })
}

fn configured_env(key: &str) -> Option<String> {
    match key {
        "OAR_FEISHU_APP_ID" => Some("cli_test".to_string()),
        "OAR_FEISHU_APP_SECRET" => Some("super-secret".to_string()),
        "OAR_FEISHU_REDIRECT_URI" => {
            Some("https://oar.example.test/auth/feishu/callback".to_string())
        }
        _ => None,
    }
}

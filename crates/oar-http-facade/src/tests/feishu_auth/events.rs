use crate::feishu_auth::{
    authorize_test_session, create_feishu_login_session, feishu_login_session_event_stream_response,
};
use crate::OarHttpFacadeRuntime;
use http_body_util::BodyExt;
use hyper::http::StatusCode;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;

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

use crate::feishu_auth::create_feishu_login_session;
use crate::OarHttpFacadeRuntime;
use hyper::http::StatusCode;
use serde_json::Value;

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
    assert!(!body["qr_page_url"]
        .as_str()
        .expect("qr url")
        .contains("im%3Amessage"));
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

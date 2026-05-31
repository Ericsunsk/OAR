use serde_json::json;

use crate::config::FeishuOpenApiConfig;
use crate::minutes::{
    build_get_minute_request, build_search_minutes_request, AsyncFeishuMinutesRead,
    FeishuMinuteReadRequest, FeishuMinuteSearchRequest, FeishuMinutesReadClient,
    FeishuMinutesReadError, MinuteReadSummary,
};
use crate::oauth::HttpResponse;
use crate::redaction::SecretString;
use crate::test_support::http::FakeHttpClient;

#[test]
fn request_builder_redacts_token_and_uses_expected_path() {
    let access_token = SecretString::new("u-very-secret-minute-token");
    let request = build_get_minute_request(
        &FeishuOpenApiConfig::default(),
        &access_token,
        "obcnq3b9jl72l83w4f14xxxx",
    )
    .expect("request");

    assert_eq!(request.method, "GET");
    assert!(request
        .url
        .ends_with("/open-apis/minutes/v1/minutes/obcnq3b9jl72l83w4f14xxxx"));
    assert!(request.headers.iter().any(|(name, value)| {
        name == "Authorization" && value == "Bearer u-very-secret-minute-token"
    }));

    let debug = format!("{request:?}");
    assert!(!debug.contains("u-very-secret-minute-token"));
    assert!(!debug.contains("obcnq3b9"));
    assert!(debug.contains("[REDACTED]"));
}

#[test]
fn search_request_builder_redacts_filters_and_uses_expected_shape() {
    let access_token = SecretString::new("u-very-secret-minute-token");
    let request = build_search_minutes_request(
        &FeishuOpenApiConfig::default(),
        FeishuMinuteSearchRequest {
            user_access_token: access_token,
            page_size: Some(99),
            page_token: Some("next_page_token".to_string()),
            query: Some(" Weekly ".to_string()),
            owner_ids: vec!["ou_owner".to_string()],
            participant_ids: vec![],
        },
    )
    .expect("request");

    assert_eq!(request.method, "POST");
    assert!(request
        .url
        .ends_with("/open-apis/minutes/v1/minutes/search?page_size=30&page_token=next_page_token"));
    assert_eq!(
        request.body,
        json!({"query":"Weekly","filter":{"owner_ids":["ou_owner"]}})
    );

    let debug = format!("{request:?}");
    assert!(!debug.contains("u-very-secret-minute-token"));
    assert!(!debug.contains("ou_owner"));
    assert!(!debug.contains("Weekly"));
    assert!(!debug.contains("next_page_token"));
}

#[test]
fn search_request_requires_filter_and_rejects_unsafe_inputs() {
    let base_request = FeishuMinuteSearchRequest {
        user_access_token: SecretString::new("u-token"),
        page_size: Some(15),
        page_token: None,
        query: None,
        owner_ids: vec![],
        participant_ids: vec![],
    };

    assert_eq!(
        build_search_minutes_request(&FeishuOpenApiConfig::default(), base_request.clone())
            .expect_err("missing filter"),
        FeishuMinutesReadError::InvalidRequest
    );

    let mut long_query_request = base_request.clone();
    long_query_request.query = Some("x".repeat(51));
    assert_eq!(
        build_search_minutes_request(&FeishuOpenApiConfig::default(), long_query_request)
            .expect_err("long query"),
        FeishuMinutesReadError::InvalidRequest
    );

    let mut unsafe_request = base_request;
    unsafe_request.owner_ids = vec!["ou_owner/unsafe".to_string()];
    assert_eq!(
        build_search_minutes_request(&FeishuOpenApiConfig::default(), unsafe_request)
            .expect_err("unsafe owner"),
        FeishuMinutesReadError::InvalidRequest
    );
}

#[test]
fn minute_request_debug_redacts_source_ref() {
    let request = FeishuMinuteReadRequest {
        user_access_token: SecretString::new("u-very-secret-minute-token"),
        source_ref: "minutes://obcnq3b9jl72l83w4f14xxxx".to_string(),
    };

    let debug = format!("{request:?}");
    assert!(!debug.contains("u-very-secret-minute-token"));
    assert!(!debug.contains("obcnq3b9"));
    assert!(debug.contains("[REDACTED]"));
}

#[test]
fn minute_search_request_debug_redacts_filters() {
    let request = FeishuMinuteSearchRequest {
        user_access_token: SecretString::new("u-very-secret-minute-token"),
        page_size: Some(15),
        page_token: Some("next_page_token".to_string()),
        query: Some("secret planning".to_string()),
        owner_ids: vec!["ou_secret_owner".to_string()],
        participant_ids: vec![],
    };

    let debug = format!("{request:?}");
    assert!(!debug.contains("u-very-secret-minute-token"));
    assert!(!debug.contains("next_page_token"));
    assert!(!debug.contains("secret planning"));
    assert!(!debug.contains("ou_secret_owner"));
    assert!(debug.contains("[REDACTED]"));
}

#[test]
fn sync_client_reads_minute_metadata() {
    let mut client = FeishuMinutesReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({"code":0,"data":{"minute":{"title":"Weekly Sync","duration":"314000","create_time":"1669098360477","token":"obcnsecret","url":"https://sample.feishu.cn/minutes/obcnsecret","owner_id":"ou_secret"}}})
                .to_string(),
        )),
    );

    let summary = client
        .get_minute_summary(FeishuMinuteReadRequest {
            user_access_token: SecretString::new("u-token"),
            source_ref: "minutes://obcnq3b9jl72l83w4f14xxxx".to_string(),
        })
        .expect("summary");

    assert_eq!(
        summary,
        MinuteReadSummary {
            title: Some("Weekly Sync".to_string()),
            create_time_ms: Some("1669098360477".to_string()),
            duration_ms: Some("314000".to_string()),
        }
    );
    assert_eq!(client.http_client().requests.len(), 1);
}

#[test]
fn sync_client_searches_minute_metadata() {
    let mut client = FeishuMinutesReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({"code":0,"data":{"items":[{"title":"Owned Weekly","duration":"314000","create_time":"1669098360477","token":"obcnsecret","url":"https://sample.feishu.cn/minutes/obcnsecret","owner_id":"ou_secret"}],"total":1,"has_more":false}})
                .to_string(),
        )),
    );

    let page = client
        .search_minute_summaries(FeishuMinuteSearchRequest {
            user_access_token: SecretString::new("u-token"),
            page_size: Some(15),
            page_token: None,
            query: None,
            owner_ids: vec!["ou_owner".to_string()],
            participant_ids: vec![],
        })
        .expect("page");

    assert_eq!(page.total, Some(1));
    assert_eq!(page.minutes.len(), 1);
    assert_eq!(page.minutes[0].title.as_deref(), Some("Owned Weekly"));
    assert_eq!(client.http_client().requests.len(), 1);
}

#[test]
fn client_rejects_invalid_source_ref_before_http() {
    let mut client = FeishuMinutesReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(200, "{}")),
    );

    let error = client
        .get_minute_summary(FeishuMinuteReadRequest {
            user_access_token: SecretString::new("u-token"),
            source_ref: "minutes://enterprise-weekly-sync".to_string(),
        })
        .expect_err("invalid");

    assert_eq!(error, FeishuMinutesReadError::InvalidSourceRef);
    assert!(client.http_client().requests.is_empty());
}

#[test]
fn async_client_reads_minute_metadata() {
    let mut client = FeishuMinutesReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({"code":0,"data":{"minute":{"title":"Async Weekly","duration":"1000","create_time":"1669098360477"}}})
                .to_string(),
        )),
    );

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let summary = runtime
        .block_on(AsyncFeishuMinutesRead::get_minute_summary(
            &mut client,
            FeishuMinuteReadRequest {
                user_access_token: SecretString::new("u-token"),
                source_ref: "minutes://obcnq3b9jl72l83w4f14xxxx".to_string(),
            },
        ))
        .expect("summary");

    assert_eq!(summary.title.as_deref(), Some("Async Weekly"));
    assert_eq!(summary.duration_ms.as_deref(), Some("1000"));
    assert_eq!(client.http_client().requests.len(), 1);
}

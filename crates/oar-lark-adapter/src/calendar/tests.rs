use serde_json::json;

use crate::calendar::{
    AsyncFeishuCalendarRead, CalendarFreeBusyBatchRequest, CalendarUserIdType,
    FeishuCalendarReadClient, FeishuCalendarReadError,
};
use crate::config::FeishuOpenApiConfig;
use crate::oauth::{HttpClientFailure, HttpResponse};
use crate::redaction::SecretString;
use crate::test_support::http::{AsyncFakeHttpClient, FakeHttpClient};

fn sample_request() -> CalendarFreeBusyBatchRequest {
    CalendarFreeBusyBatchRequest {
        user_access_token: SecretString::new("u-very-secret-calendar-token"),
        user_ids: vec!["ou_current_user".to_string()],
        time_min: "2026-05-29T00:00:00Z".to_string(),
        time_max: "2026-05-30T00:00:00Z".to_string(),
        include_external_calendar: false,
        only_busy: true,
        need_rsvp_status: false,
        user_id_type: CalendarUserIdType::OpenId,
    }
}

#[test]
fn batch_free_busy_success_returns_sanitized_page() {
    let response = HttpResponse::new(
        200,
        json!({
            "code": 0,
            "msg": "success",
            "data": {
                "freebusy_lists": [
                    {
                        "user_id": "ou_current_user",
                        "freebusy_items": [
                            {
                                "start_time": "2026-05-29T10:00:00+08:00",
                                "end_time": "2026-05-29T11:00:00+08:00",
                                "rsvp_status": "accepted",
                                "event_title": "raw event title should not surface"
                            }
                        ]
                    },
                    {
                        "user_id": "unsafe/user",
                        "freebusy_items": [
                            {"start_time": "2026-05-29T12:00:00+08:00"}
                        ]
                    }
                ]
            }
        })
        .to_string(),
    );
    let mut client = FeishuCalendarReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(response),
    );

    let page = client.batch_free_busy(sample_request()).expect("success");

    assert_eq!(page.lists.len(), 1);
    assert_eq!(page.lists[0].busy_items.len(), 1);
    assert_eq!(
        page.lists[0].busy_items[0].start_time.as_deref(),
        Some("2026-05-29T10:00:00+08:00")
    );

    let serialized = serde_json::to_string(&page).expect("page json");
    assert!(!serialized.contains("ou_current_user"));
    assert!(!serialized.contains("raw event title"));
    assert!(!serialized.contains("unsafe/user"));
}

#[test]
fn batch_free_busy_request_contains_user_token_but_debug_redacts_it() {
    let mut client = FeishuCalendarReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({"code":0,"data":{"freebusy_lists":[]}}).to_string(),
        )),
    );

    let request = sample_request();
    let request_debug = format!("{request:?}");
    assert!(!request_debug.contains("u-very-secret-calendar-token"));
    assert!(request_debug.contains("[REDACTED]"));

    client.batch_free_busy(request).expect("success");
    let sent = client
        .http_client()
        .request
        .as_ref()
        .expect("captured request");

    assert_eq!(sent.method, "POST");
    assert!(sent
        .url
        .ends_with("/open-apis/calendar/v4/freebusy/batch?user_id_type=open_id"));
    assert_eq!(sent.body["user_ids"], json!(["ou_current_user"]));
    assert_eq!(sent.body["include_external_calendar"], json!(false));
    assert_eq!(sent.body["only_busy"], json!(true));
    assert!(sent.headers.iter().any(|(name, value)| {
        name == "Authorization" && value == "Bearer u-very-secret-calendar-token"
    }));

    let sent_debug = format!("{sent:?}");
    assert!(!sent_debug.contains("u-very-secret-calendar-token"));
    assert!(sent_debug.contains("[REDACTED]"));
}

#[test]
fn batch_free_busy_rejects_unsafe_request_shapes_before_http() {
    let response = HttpResponse::new(
        200,
        json!({"code":0,"data":{"freebusy_lists":[]}}).to_string(),
    );
    let mut client = FeishuCalendarReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(response),
    );

    let mut request = sample_request();
    request.user_ids = vec![];
    assert_eq!(
        client.batch_free_busy(request),
        Err(FeishuCalendarReadError::InvalidRequest)
    );

    let mut request = sample_request();
    request.user_ids = vec!["ou_current_user/unsafe".to_string()];
    assert_eq!(
        client.batch_free_busy(request),
        Err(FeishuCalendarReadError::InvalidRequest)
    );

    let mut request = sample_request();
    request.time_min = "not a time".to_string();
    assert_eq!(
        client.batch_free_busy(request),
        Err(FeishuCalendarReadError::InvalidRequest)
    );
}

#[test]
fn batch_free_busy_maps_status_codes_and_api_codes_to_safe_errors() {
    let mut unauthorized = FeishuCalendarReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(401, "{}")),
    );
    assert_eq!(
        unauthorized.batch_free_busy(sample_request()),
        Err(FeishuCalendarReadError::Unauthorized)
    );

    let mut forbidden = FeishuCalendarReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(403, "{}")),
    );
    assert_eq!(
        forbidden.batch_free_busy(sample_request()),
        Err(FeishuCalendarReadError::Forbidden)
    );

    let mut rate_limited = FeishuCalendarReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(429, "{}")),
    );
    assert_eq!(
        rate_limited.batch_free_busy(sample_request()),
        Err(FeishuCalendarReadError::UpstreamTransient)
    );

    let mut invalid_time = FeishuCalendarReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({
                "code": 198002,
                "msg": "invalid time with sensitive details",
                "data": {"debug_token": "u-very-secret-calendar-token"}
            })
            .to_string(),
        )),
    );
    assert_eq!(
        invalid_time.batch_free_busy(sample_request()),
        Err(FeishuCalendarReadError::UpstreamClient)
    );
    assert!(!format!("{:?}", FeishuCalendarReadError::UpstreamClient)
        .contains("u-very-secret-calendar-token"));
}

#[test]
fn batch_free_busy_fail_closed_for_invalid_json_missing_data_and_transport() {
    let mut invalid_json = FeishuCalendarReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(200, "{not-json")),
    );
    assert_eq!(
        invalid_json.batch_free_busy(sample_request()),
        Err(FeishuCalendarReadError::InvalidJson)
    );

    let mut missing_data = FeishuCalendarReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(200, json!({"code":0}).to_string())),
    );
    assert_eq!(
        missing_data.batch_free_busy(sample_request()),
        Err(FeishuCalendarReadError::InvalidJson)
    );

    let mut oversized = FeishuCalendarReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_error(HttpClientFailure::OversizedResponse {
            max_response_bytes: 16,
        }),
    );
    assert_eq!(
        oversized.batch_free_busy(sample_request()),
        Err(FeishuCalendarReadError::OversizedResponse)
    );

    let mut transport = FeishuCalendarReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_error(HttpClientFailure::Transport),
    );
    assert_eq!(
        transport.batch_free_busy(sample_request()),
        Err(FeishuCalendarReadError::Transport)
    );
}

#[test]
fn async_batch_free_busy_success_response_parses() {
    let response = HttpResponse::new(
        200,
        json!({
            "code": 0,
            "data": {
                "freebusy_lists": [
                    {
                        "user_id": "ou_current_user",
                        "freebusy_items": [
                            {"start_time": "2026-05-29T10:00:00Z", "end_time": "2026-05-29T11:00:00Z"}
                        ]
                    }
                ]
            }
        })
        .to_string(),
    );
    let mut client = FeishuCalendarReadClient::new(
        FeishuOpenApiConfig::default(),
        AsyncFakeHttpClient { response },
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let parsed = runtime
        .block_on(client.batch_free_busy(sample_request()))
        .expect("success");

    assert_eq!(parsed.lists[0].busy_items.len(), 1);
}

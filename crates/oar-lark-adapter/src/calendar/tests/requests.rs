use serde_json::json;

use super::support::sample_request;
use crate::calendar::{FeishuCalendarReadClient, FeishuCalendarReadError};
use crate::config::FeishuOpenApiConfig;
use crate::oauth::HttpResponse;
use crate::test_support::http::FakeHttpClient;

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

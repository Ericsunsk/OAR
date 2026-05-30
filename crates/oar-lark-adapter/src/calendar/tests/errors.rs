use serde_json::json;

use super::support::sample_request;
use crate::calendar::{FeishuCalendarReadClient, FeishuCalendarReadError};
use crate::config::FeishuOpenApiConfig;
use crate::oauth::{HttpClientFailure, HttpResponse};
use crate::test_support::http::FakeHttpClient;

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

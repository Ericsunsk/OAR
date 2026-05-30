use super::{sample_request, FakeHttpClient};
use crate::config::FeishuOpenApiConfig;
use crate::oauth::{HttpClientFailure, HttpResponse};
use crate::okr::{FeishuOkrReadClient, FeishuOkrReadError};

#[test]
fn batch_get_maps_status_codes_to_safe_errors() {
    let mut unauthorized = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(401, "{}")),
    );
    assert_eq!(
        unauthorized.batch_get_okrs(sample_request()),
        Err(FeishuOkrReadError::Unauthorized)
    );

    let mut forbidden = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(403, "{}")),
    );
    assert_eq!(
        forbidden.batch_get_okrs(sample_request()),
        Err(FeishuOkrReadError::Forbidden)
    );

    let mut server_error = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(503, "{}")),
    );
    assert_eq!(
        server_error.batch_get_okrs(sample_request()),
        Err(FeishuOkrReadError::UpstreamTransient)
    );
}

#[test]
fn batch_get_fail_closed_for_oversized_and_invalid_json() {
    let mut oversized = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_error(HttpClientFailure::OversizedResponse {
            max_response_bytes: 32,
        }),
    );
    assert_eq!(
        oversized.batch_get_okrs(sample_request()),
        Err(FeishuOkrReadError::OversizedResponse)
    );

    let mut invalid_json = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(200, "{not-json")),
    );
    assert_eq!(
        invalid_json.batch_get_okrs(sample_request()),
        Err(FeishuOkrReadError::InvalidJson)
    );
}

#[test]
fn token_is_redacted_in_okr_request_debug_and_errors() {
    let request = sample_request();
    let debug = format!("{request:?}");
    assert!(!debug.contains("u-very-secret-token"));
    assert!(debug.contains("[REDACTED]"));

    let error_debug = format!("{:?}", FeishuOkrReadError::Unauthorized);
    let error_display = FeishuOkrReadError::Unauthorized.to_string();
    assert!(!error_debug.contains("u-very-secret-token"));
    assert!(!error_display.contains("u-very-secret-token"));
}

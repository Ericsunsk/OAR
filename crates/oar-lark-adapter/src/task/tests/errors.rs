use serde_json::json;

use super::sample_request;
use crate::config::FeishuOpenApiConfig;
use crate::oauth::{HttpClientFailure, HttpResponse};
use crate::task::{FeishuTaskReadClient, FeishuTaskReadError};
use crate::test_support::http::FakeHttpClient;

#[test]
fn get_task_maps_status_codes_to_safe_errors() {
    let mut unauthorized = FeishuTaskReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(401, "{}")),
    );
    assert_eq!(
        unauthorized.get_task_summary(sample_request()),
        Err(FeishuTaskReadError::Unauthorized)
    );

    let mut forbidden = FeishuTaskReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(403, "{}")),
    );
    assert_eq!(
        forbidden.get_task_summary(sample_request()),
        Err(FeishuTaskReadError::Forbidden)
    );

    let mut not_found = FeishuTaskReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(404, "{}")),
    );
    assert_eq!(
        not_found.get_task_summary(sample_request()),
        Err(FeishuTaskReadError::NotFound)
    );

    let mut upstream_client = FeishuTaskReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(429, "{}")),
    );
    assert_eq!(
        upstream_client.get_task_summary(sample_request()),
        Err(FeishuTaskReadError::UpstreamClient)
    );

    let mut server_error = FeishuTaskReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(503, "{}")),
    );
    assert_eq!(
        server_error.get_task_summary(sample_request()),
        Err(FeishuTaskReadError::UpstreamTransient)
    );
}

#[test]
fn get_task_maps_api_codes_without_exposing_payload() {
    let mut forbidden = FeishuTaskReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({
                "code": 1470403,
                "msg": "permission denied with sensitive upstream details",
                "data": {"debug_token": "u-very-secret-task-token"}
            })
            .to_string(),
        )),
    );

    assert_eq!(
        forbidden.get_task_summary(sample_request()),
        Err(FeishuTaskReadError::Forbidden)
    );
    assert!(!format!("{:?}", FeishuTaskReadError::Forbidden).contains("u-very-secret-task-token"));
}

#[test]
fn get_task_fail_closed_for_oversized_invalid_json_and_missing_task() {
    let mut oversized = FeishuTaskReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_error(HttpClientFailure::OversizedResponse {
            max_response_bytes: 32,
        }),
    );
    assert_eq!(
        oversized.get_task_summary(sample_request()),
        Err(FeishuTaskReadError::OversizedResponse)
    );

    let mut invalid_json = FeishuTaskReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(200, "{not-json")),
    );
    assert_eq!(
        invalid_json.get_task_summary(sample_request()),
        Err(FeishuTaskReadError::InvalidJson)
    );

    let mut missing_task = FeishuTaskReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({"code":0,"data":{}}).to_string(),
        )),
    );
    assert_eq!(
        missing_task.get_task_summary(sample_request()),
        Err(FeishuTaskReadError::InvalidJson)
    );
}

#[test]
fn list_task_maps_status_codes_and_fail_closed_shapes() {
    let mut forbidden = FeishuTaskReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(403, "{}")),
    );
    assert_eq!(
        forbidden.list_task_summaries(super::sample_list_request()),
        Err(FeishuTaskReadError::Forbidden)
    );

    let mut invalid_json = FeishuTaskReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(200, "{not-json")),
    );
    assert_eq!(
        invalid_json.list_task_summaries(super::sample_list_request()),
        Err(FeishuTaskReadError::InvalidJson)
    );

    let mut missing_data = FeishuTaskReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(200, json!({"code":0}).to_string())),
    );
    assert_eq!(
        missing_data.list_task_summaries(super::sample_list_request()),
        Err(FeishuTaskReadError::InvalidJson)
    );
}

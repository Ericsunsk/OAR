use async_trait::async_trait;
use serde_json::json;

use crate::config::FeishuOpenApiConfig;
use crate::oauth::{AsyncHttpClient, HttpClient, HttpClientFailure, HttpRequest, HttpResponse};
use crate::okr::{
    AsyncFeishuOkrRead, FeishuOkrBatchGetRequest, FeishuOkrReadClient, FeishuOkrReadError,
    OkrUserIdType,
};
use crate::redaction::SecretString;

#[derive(Clone)]
struct FakeHttpClient {
    response: Option<HttpResponse>,
    error: Option<HttpClientFailure>,
    request: Option<HttpRequest>,
}

impl FakeHttpClient {
    fn from_response(response: HttpResponse) -> Self {
        Self {
            response: Some(response),
            error: None,
            request: None,
        }
    }

    fn from_error(error: HttpClientFailure) -> Self {
        Self {
            response: None,
            error: Some(error),
            request: None,
        }
    }
}

impl HttpClient for FakeHttpClient {
    fn post_json(&mut self, request: HttpRequest) -> Result<HttpResponse, HttpClientFailure> {
        self.request = Some(request);
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        Ok(self.response.clone().expect("response exists"))
    }
}

#[derive(Clone)]
struct AsyncFakeHttpClient {
    response: HttpResponse,
}

#[async_trait]
impl AsyncHttpClient for AsyncFakeHttpClient {
    async fn post_json(
        &mut self,
        _request: HttpRequest,
    ) -> Result<HttpResponse, HttpClientFailure> {
        Ok(self.response.clone())
    }
}

fn sample_request() -> FeishuOkrBatchGetRequest {
    FeishuOkrBatchGetRequest {
        user_access_token: SecretString::new("u-very-secret-token"),
        user_id_type: OkrUserIdType::OpenId,
        okr_ids: vec!["okr_1".to_string(), "okr_2".to_string()],
        lang: Some("zh_cn".to_string()),
    }
}

#[test]
fn batch_get_success_response_parses() {
    let response = HttpResponse::new(
        200,
        json!({
            "code": 0,
            "msg": "ok",
            "data": {
                "okr_list": [
                    {"okr_id":"okr_1","name":"A"},
                    {"okr_id":"okr_2","name":"B"}
                ]
            }
        })
        .to_string(),
    );
    let mut client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(response),
    );
    let parsed = client.batch_get_okrs(sample_request()).expect("success");
    assert_eq!(parsed.code, 0);
    assert_eq!(parsed.data.expect("data").okr_list.len(), 2);
}

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

#[test]
fn batch_get_request_uses_get_and_query_parameters() {
    let mut client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({"code":0,"data":{"okr_list":[]}}).to_string(),
        )),
    );
    client.batch_get_okrs(sample_request()).expect("success");
    let sent = client
        .http_client()
        .request
        .as_ref()
        .expect("captured request");
    assert_eq!(sent.method, "GET");
    assert!(sent.url.contains("user_id_type=open_id"));
    assert!(sent.url.contains("okr_ids=okr_1"));
    assert!(sent.url.contains("okr_ids=okr_2"));
    assert!(sent.url.contains("lang=zh_cn"));
    assert_eq!(sent.body, json!({}));
    let debug = format!("{sent:?}");
    assert!(!debug.contains("u-very-secret-token"));
    assert!(!debug.contains("Bearer u-very-secret-token"));
    assert!(debug.contains("[REDACTED]"));
}

#[test]
fn batch_get_rejects_more_than_ten_okr_ids() {
    let mut client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({"code":0,"data":{"okr_list":[]}}).to_string(),
        )),
    );
    let mut request = sample_request();
    request.okr_ids = (0..11).map(|i| format!("okr_{i}")).collect();
    assert_eq!(
        client.batch_get_okrs(request),
        Err(FeishuOkrReadError::InvalidRequest)
    );
}

#[test]
fn async_batch_get_success_response_parses() {
    let response = HttpResponse::new(
        200,
        json!({
            "code": 0,
            "data": { "okr_list": [{"okr_id":"okr_1"}] }
        })
        .to_string(),
    );
    let mut client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        AsyncFakeHttpClient { response },
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let parsed = runtime
        .block_on(client.batch_get_okrs(sample_request()))
        .expect("success");
    assert_eq!(parsed.code, 0);
}

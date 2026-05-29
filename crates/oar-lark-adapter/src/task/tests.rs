use async_trait::async_trait;
use serde_json::json;

use crate::config::FeishuOpenApiConfig;
use crate::oauth::{AsyncHttpClient, HttpClient, HttpClientFailure, HttpRequest, HttpResponse};
use crate::redaction::SecretString;
use crate::task::{
    parse_task_source_ref, AsyncFeishuTaskRead, FeishuTaskGetRequest, FeishuTaskReadClient,
    FeishuTaskReadError, TaskUserIdType,
};

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

fn sample_request() -> FeishuTaskGetRequest {
    FeishuTaskGetRequest {
        user_access_token: SecretString::new("u-very-secret-task-token"),
        source_ref: "task://task_123".to_string(),
        user_id_type: TaskUserIdType::OpenId,
    }
}

#[test]
fn source_ref_parser_accepts_task_and_feishu_task_schemes() {
    let parsed = parse_task_source_ref(" task://task_123 ").expect("source ref");
    assert_eq!(parsed.task_id, "task_123");

    let feishu = parse_task_source_ref("feishu://task/task_456").expect("source ref");
    assert_eq!(feishu.task_id, "task_456");

    assert_eq!(
        parse_task_source_ref("okr://okr_1"),
        Err(FeishuTaskReadError::InvalidSourceRef)
    );
    assert_eq!(
        parse_task_source_ref("task://"),
        Err(FeishuTaskReadError::InvalidSourceRef)
    );
    assert_eq!(
        parse_task_source_ref("task://task_123/subtask"),
        Err(FeishuTaskReadError::InvalidSourceRef)
    );
}

#[test]
fn get_task_success_returns_sanitized_summary() {
    let response = HttpResponse::new(
        200,
        json!({
            "code": 0,
            "msg": "ok",
            "data": {
                "task": {
                    "guid": "task_123",
                    "summary": " Ship task read adapter ",
                    "status": 2,
                    "due": {
                        "timestamp": "1780000000000",
                        "is_all_day": true,
                        "timezone": "Asia/Shanghai"
                    },
                    "members": [
                        {
                            "member_id": "ou_owner",
                            "member_type": "open_id",
                            "role": "assignee",
                            "name": "raw payload name should not surface"
                        }
                    ],
                    "updated_at": 1781000000000_i64,
                    "description": "raw body field should not surface"
                }
            }
        })
        .to_string(),
    );
    let mut client = FeishuTaskReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(response),
    );

    let summary = client.get_task_summary(sample_request()).expect("success");

    assert_eq!(summary.source_ref, "task://task_123");
    assert_eq!(summary.task_id, "task_123");
    assert_eq!(summary.title.as_deref(), Some("Ship task read adapter"));
    assert_eq!(summary.status.as_deref(), Some("2"));
    assert_eq!(
        summary
            .due
            .as_ref()
            .and_then(|due| due.timestamp.as_deref()),
        Some("1780000000000")
    );
    assert_eq!(
        summary.due.as_ref().and_then(|due| due.is_all_day),
        Some(true)
    );
    assert_eq!(summary.owners.len(), 1);
    assert_eq!(summary.owners[0].owner_id.as_deref(), Some("ou_owner"));
    assert_eq!(summary.owners[0].owner_type.as_deref(), Some("open_id"));
    assert_eq!(summary.update_time.as_deref(), Some("1781000000000"));

    let serialized = serde_json::to_string(&summary).expect("summary json");
    assert!(!serialized.contains("description"));
    assert!(!serialized.contains("raw payload"));
    assert!(!serialized.contains("timezone"));
}

#[test]
fn get_task_tolerates_missing_optional_fields_and_shape_variants() {
    let response = HttpResponse::new(
        200,
        json!({
            "code": 0,
            "data": {
                "task": {
                    "task_id": "task_123",
                    "name": "",
                    "completed": false,
                    "due": 1780000000000_i64,
                    "creator": {
                        "open_id": "ou_creator",
                        "type": "open_id"
                    },
                    "update_time": "2026-05-20T10:00:00Z"
                }
            }
        })
        .to_string(),
    );
    let mut client = FeishuTaskReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(response),
    );

    let summary = client.get_task_summary(sample_request()).expect("success");

    assert_eq!(summary.title, None);
    assert_eq!(summary.status.as_deref(), Some("open"));
    assert_eq!(
        summary
            .due
            .as_ref()
            .and_then(|due| due.timestamp.as_deref()),
        Some("1780000000000")
    );
    assert_eq!(summary.owners[0].owner_id.as_deref(), Some("ou_creator"));
    assert_eq!(summary.update_time.as_deref(), Some("2026-05-20T10:00:00Z"));
}

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
fn get_task_request_contains_bearer_token_but_debug_redacts_it() {
    let request = sample_request();
    let request_debug = format!("{request:?}");
    assert!(!request_debug.contains("u-very-secret-task-token"));
    assert!(request_debug.contains("[REDACTED]"));

    let mut client = FeishuTaskReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({"code":0,"data":{"task":{"guid":"task_123"}}}).to_string(),
        )),
    );

    client.get_task_summary(request).expect("success");
    let sent = client
        .http_client()
        .request
        .as_ref()
        .expect("captured request");

    assert_eq!(sent.method, "GET");
    assert!(sent
        .url
        .ends_with("/open-apis/task/v2/tasks/task_123?user_id_type=open_id"));
    assert_eq!(sent.body, json!({}));
    assert!(sent.headers.iter().any(|(name, value)| {
        name == "Authorization" && value == "Bearer u-very-secret-task-token"
    }));

    let debug = format!("{sent:?}");
    assert!(!debug.contains("u-very-secret-task-token"));
    assert!(!debug.contains("Bearer u-very-secret-task-token"));
    assert!(debug.contains("[REDACTED]"));
}

#[test]
fn async_get_task_success_response_parses() {
    let response = HttpResponse::new(
        200,
        json!({
            "code": 0,
            "data": { "task": {"guid":"task_123", "summary":"async task"} }
        })
        .to_string(),
    );
    let mut client = FeishuTaskReadClient::new(
        FeishuOpenApiConfig::default(),
        AsyncFakeHttpClient { response },
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let parsed = runtime
        .block_on(client.get_task_summary(sample_request()))
        .expect("success");
    assert_eq!(parsed.title.as_deref(), Some("async task"));
}

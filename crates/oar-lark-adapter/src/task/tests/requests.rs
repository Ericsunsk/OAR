use serde_json::json;

use super::sample_request;
use crate::config::FeishuOpenApiConfig;
use crate::oauth::HttpResponse;
use crate::redaction::SecretString;
use crate::task::{FeishuTaskListRequest, FeishuTaskReadClient, TaskListType, TaskUserIdType};
use crate::test_support::http::FakeHttpClient;

#[test]
fn list_tasks_request_contains_safe_query_and_redacts_token_in_debug() {
    let mut client = FeishuTaskReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({"code":0,"data":{"items":[]}}).to_string(),
        )),
    );

    client
        .list_task_summaries(FeishuTaskListRequest {
            user_access_token: SecretString::new("u-very-secret-task-token"),
            page_size: Some(250),
            page_token: Some("next/page token".to_string()),
            completed: Some(true),
            task_type: TaskListType::MyTasks,
            user_id_type: TaskUserIdType::OpenId,
        })
        .expect("success");
    let sent = client
        .http_client()
        .request
        .as_ref()
        .expect("captured request");

    assert_eq!(sent.method, "GET");
    assert!(sent.url.contains("/open-apis/task/v2/tasks?"));
    assert!(sent.url.contains("page_size=100"));
    assert!(sent.url.contains("page_token=next%2Fpage%20token"));
    assert!(sent.url.contains("completed=true"));
    assert!(sent.url.contains("type=my_tasks"));
    assert!(sent.url.contains("user_id_type=open_id"));
    assert!(sent.headers.iter().any(|(name, value)| {
        name == "Authorization" && value == "Bearer u-very-secret-task-token"
    }));

    let request_debug = format!("{sent:?}");
    assert!(!request_debug.contains("u-very-secret-task-token"));
    assert!(request_debug.contains("[REDACTED]"));
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

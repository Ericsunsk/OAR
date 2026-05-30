use serde_json::json;

use super::sample_request;
use crate::config::FeishuOpenApiConfig;
use crate::oauth::HttpResponse;
use crate::task::{AsyncFeishuTaskRead, FeishuTaskReadClient};
use crate::test_support::http::{AsyncFakeHttpClient, FakeHttpClient};

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

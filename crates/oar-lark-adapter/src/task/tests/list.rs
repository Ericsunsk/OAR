use serde_json::json;

use super::sample_list_request;
use crate::config::FeishuOpenApiConfig;
use crate::oauth::HttpResponse;
use crate::task::FeishuTaskReadClient;
use crate::test_support::http::FakeHttpClient;

#[test]
fn list_tasks_success_returns_sanitized_page() {
    let response = HttpResponse::new(
        200,
        json!({
            "code": 0,
            "msg": "ok",
            "data": {
                "items": [
                    {
                        "guid": "task_123",
                        "summary": " Ship task list adapter ",
                        "completed": false,
                        "due": {"timestamp": "1780000000000", "is_all_day": true},
                        "members": [
                            {"member_id": "ou_owner", "member_type": "open_id", "role": "assignee"}
                        ],
                        "updated_at": 1781000000000_i64,
                        "description": "raw body field should not surface"
                    },
                    {
                        "guid": "task_unsafe/ref",
                        "summary": "invalid id is skipped"
                    },
                    {
                        "guid": "task_456",
                        "summary": "Second task",
                        "completed": true
                    }
                ],
                "has_more": true,
                "page_token": "next-page"
            }
        })
        .to_string(),
    );
    let mut client = FeishuTaskReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(response),
    );

    let page = client
        .list_task_summaries(sample_list_request())
        .expect("success");

    assert_eq!(page.tasks.len(), 2);
    assert_eq!(page.tasks[0].source_ref, "task://task_123");
    assert_eq!(
        page.tasks[0].title.as_deref(),
        Some("Ship task list adapter")
    );
    assert_eq!(page.tasks[0].status.as_deref(), Some("open"));
    assert_eq!(page.tasks[0].owners.len(), 1);
    assert_eq!(page.tasks[1].source_ref, "task://task_456");
    assert_eq!(page.tasks[1].status.as_deref(), Some("completed"));
    assert!(page.has_more);
    assert_eq!(page.page_token.as_deref(), Some("next-page"));

    let serialized = serde_json::to_string(&page).expect("page json");
    assert!(!serialized.contains("description"));
    assert!(!serialized.contains("raw body"));
    assert!(!serialized.contains("task_unsafe/ref"));
}

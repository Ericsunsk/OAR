use serde_json::json;

use super::{sample_progress_list_request, FakeHttpClient};
use crate::config::FeishuOpenApiConfig;
use crate::oauth::HttpResponse;
use crate::okr::{FeishuOkrProgressListTarget, FeishuOkrReadClient, OkrReadProgressPage};

#[test]
fn progress_list_success_parses_progress_list_and_safe_page_is_sanitized() {
    let response = HttpResponse::new(
        200,
        json!({
            "code": 0,
            "msg": "ok",
            "data": {
                "progress_list": [
                    {
                        "progress_id": "pr_1",
                        "modify_time": 1780000000000_i64,
                        "content": {"text": "private progress body"},
                        "person": {"name": "Alice Example"},
                        "image_list": [{"file_token": "img_secret"}],
                        "progress_rate": {"percent": 75.5, "status": "normal"}
                    },
                    {
                        "id": 12345,
                        "modify_time": "2026-05-20T10:00:00Z",
                        "progress_rate": {"percent": "80", "status": 2}
                    }
                ],
                "next_page_token": "next-progress",
                "has_more": true
            }
        })
        .to_string(),
    );
    let mut client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(response),
    );
    let parsed = client
        .list_progress(sample_progress_list_request(
            FeishuOkrProgressListTarget::Objective("obj_1".to_string()),
        ))
        .expect("success");
    assert_eq!(parsed.code, 0);
    let data = parsed.data.expect("data");
    assert_eq!(data.progress_list.len(), 2);
    assert!(data.progress_list[0].extra.contains_key("content"));
    assert!(data.progress_list[0].extra.contains_key("person"));
    assert_eq!(data.progress_list[0].progress_id.as_deref(), Some("pr_1"));
    assert_eq!(data.progress_list[1].progress_id.as_deref(), Some("12345"));

    let page = OkrReadProgressPage::from_progress_list_data(&data);
    assert_eq!(page.progress_records.len(), 2);
    assert_eq!(page.next_page_token.as_deref(), Some("next-progress"));
    assert!(page.has_more);
    assert_eq!(page.progress_records[0].id.as_deref(), Some("pr_1"));
    assert_eq!(
        page.progress_records[0].modify_time.as_deref(),
        Some("1780000000000")
    );
    assert_eq!(page.progress_records[0].percent.as_deref(), Some("75.5"));
    assert_eq!(page.progress_records[0].status.as_deref(), Some("normal"));
    assert_eq!(page.progress_records[1].id.as_deref(), Some("12345"));
    assert_eq!(page.progress_records[1].status.as_deref(), Some("2"));

    let safe_json = serde_json::to_string(&page).expect("safe json");
    assert!(!safe_json.contains("private progress body"));
    assert!(!safe_json.contains("Alice Example"));
    assert!(!safe_json.contains("img_secret"));
    assert!(!safe_json.contains("content"));
    assert!(!safe_json.contains("person"));
    assert!(!safe_json.contains("image_list"));
}

use serde_json::json;

use super::support::sample_request;
use crate::calendar::FeishuCalendarReadClient;
use crate::config::FeishuOpenApiConfig;
use crate::oauth::HttpResponse;
use crate::test_support::http::FakeHttpClient;

#[test]
fn batch_free_busy_success_returns_sanitized_page() {
    let response = HttpResponse::new(
        200,
        json!({
            "code": 0,
            "msg": "success",
            "data": {
                "freebusy_lists": [
                    {
                        "user_id": "ou_current_user",
                        "freebusy_items": [
                            {
                                "start_time": "2026-05-29T10:00:00+08:00",
                                "end_time": "2026-05-29T11:00:00+08:00",
                                "rsvp_status": "accepted",
                                "event_title": "raw event title should not surface"
                            }
                        ]
                    },
                    {
                        "user_id": "unsafe/user",
                        "freebusy_items": [
                            {"start_time": "2026-05-29T12:00:00+08:00"}
                        ]
                    }
                ]
            }
        })
        .to_string(),
    );
    let mut client = FeishuCalendarReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(response),
    );

    let page = client.batch_free_busy(sample_request()).expect("success");

    assert_eq!(page.lists.len(), 1);
    assert_eq!(page.lists[0].busy_items.len(), 1);
    assert_eq!(
        page.lists[0].busy_items[0].start_time.as_deref(),
        Some("2026-05-29T10:00:00+08:00")
    );

    let serialized = serde_json::to_string(&page).expect("page json");
    assert!(!serialized.contains("ou_current_user"));
    assert!(!serialized.contains("raw event title"));
    assert!(!serialized.contains("unsafe/user"));
}

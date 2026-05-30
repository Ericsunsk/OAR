use serde_json::json;

use super::support::sample_request;
use crate::calendar::{AsyncFeishuCalendarRead, FeishuCalendarReadClient};
use crate::config::FeishuOpenApiConfig;
use crate::oauth::HttpResponse;
use crate::test_support::http::AsyncFakeHttpClient;

#[test]
fn async_batch_free_busy_success_response_parses() {
    let response = HttpResponse::new(
        200,
        json!({
            "code": 0,
            "data": {
                "freebusy_lists": [
                    {
                        "user_id": "ou_current_user",
                        "freebusy_items": [
                            {"start_time": "2026-05-29T10:00:00Z", "end_time": "2026-05-29T11:00:00Z"}
                        ]
                    }
                ]
            }
        })
        .to_string(),
    );
    let mut client = FeishuCalendarReadClient::new(
        FeishuOpenApiConfig::default(),
        AsyncFakeHttpClient { response },
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let parsed = runtime
        .block_on(client.batch_free_busy(sample_request()))
        .expect("success");

    assert_eq!(parsed.lists[0].busy_items.len(), 1);
}

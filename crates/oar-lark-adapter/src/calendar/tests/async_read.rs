use serde_json::json;

use super::support::{
    sample_event_read_request, sample_instance_view_request, sample_primary_request, sample_request,
};
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

#[test]
fn async_primary_and_instance_view_success_response_parse() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");

    let mut primary = FeishuCalendarReadClient::new(
        FeishuOpenApiConfig::default(),
        AsyncFakeHttpClient {
            response: HttpResponse::new(
                200,
                json!({"code":0,"data":{"calendars":[{"calendar":{"calendar_id":"cal_primary"}}]}})
                    .to_string(),
            ),
        },
    );
    let parsed_primary = runtime
        .block_on(primary.primary_calendar(sample_primary_request()))
        .expect("success");
    assert_eq!(parsed_primary.calendar.calendar_id, "cal_primary");

    let mut instances = FeishuCalendarReadClient::new(
        FeishuOpenApiConfig::default(),
        AsyncFakeHttpClient {
            response: HttpResponse::new(
                200,
                json!({"code":0,"data":{"instances":[{"event_id":"evt_1","attendees":[{},{}]}]}})
                    .to_string(),
            ),
        },
    );
    let parsed_instances = runtime
        .block_on(instances.event_instance_view(sample_instance_view_request()))
        .expect("success");
    assert_eq!(parsed_instances.events[0].attendee_count, 2);

    let mut event = FeishuCalendarReadClient::new(
        FeishuOpenApiConfig::default(),
        AsyncFakeHttpClient {
            response: HttpResponse::new(
                200,
                json!({"code":0,"data":{"event":{"event_id":"evt_1","summary":"Review"}}})
                    .to_string(),
            ),
        },
    );
    let parsed_event = runtime
        .block_on(event.get_event_summary(sample_event_read_request()))
        .expect("success");
    assert_eq!(parsed_event.summary.as_deref(), Some("Review"));
}

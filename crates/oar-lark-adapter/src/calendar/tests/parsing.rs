use serde_json::json;

use super::support::{
    sample_event_read_request, sample_instance_view_request, sample_primary_request, sample_request,
};
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

#[test]
fn primary_calendar_success_returns_minimal_calendar() {
    let response = HttpResponse::new(
        200,
        json!({
            "code": 0,
            "msg": "success",
            "data": {
                "calendars": [
                    {
                        "calendar": {
                            "calendar_id": "cal_primary",
                            "summary": "Primary Calendar",
                            "description": "should not surface"
                        },
                        "user_id": "ou_secret_owner"
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

    let page = client
        .primary_calendar(sample_primary_request())
        .expect("success");

    assert_eq!(page.calendar.calendar_id, "cal_primary");
    assert_eq!(page.calendar.summary.as_deref(), Some("Primary Calendar"));
    let serialized = serde_json::to_string(&page).expect("primary json");
    assert!(!serialized.contains("should not surface"));
    assert!(!serialized.contains("ou_secret_owner"));
}

#[test]
fn instance_view_success_returns_sanitized_agenda() {
    let response = HttpResponse::new(
        200,
        json!({
            "code": 0,
            "msg": "success",
            "data": {
                "instances": [
                    {
                        "event_id": "evt_1",
                        "summary": "Roadmap review",
                        "description": "private notes should not surface",
                        "meeting_url": "https://vc.example/private",
                        "app_link": "https://applink.example/private",
                        "attachments": [{"file_token": "secret_file"}],
                        "start_time": {
                            "timestamp": "1779987600",
                            "timezone": "Asia/Shanghai"
                        },
                        "end_time": {
                            "timestamp": "1779991200",
                            "timezone": "Asia/Shanghai"
                        },
                        "status": "confirmed",
                        "visibility": "default",
                        "free_busy_status": "busy",
                        "location": {
                            "name": "Room 8",
                            "address": "address should not surface"
                        },
                        "organizer": {
                            "open_id": "ou_secret_organizer",
                            "display_name": "Ada"
                        },
                        "attendees": [
                            {"open_id": "ou_secret_1", "display_name": "One"},
                            {"user_id": "u_secret_2", "display_name": "Two"}
                        ]
                    },
                    {
                        "event_id": "evt_2",
                        "summary": "No attendees",
                        "attendees": null
                    },
                    {
                        "event_id": "",
                        "summary": "missing id should be dropped",
                        "attendees": [{"open_id": "ou_secret_3"}]
                    }
                ],
            }
        })
        .to_string(),
    );
    let mut client = FeishuCalendarReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(response),
    );

    let page = client
        .event_instance_view(sample_instance_view_request())
        .expect("success");

    assert_eq!(page.events.len(), 2);
    let event = &page.events[0];
    assert_eq!(event.summary.as_deref(), Some("Roadmap review"));
    assert_eq!(
        event
            .start_time_info
            .as_ref()
            .and_then(|time| time.timestamp.as_deref()),
        Some("1779987600")
    );
    assert_eq!(
        event
            .location
            .as_ref()
            .and_then(|location| location.name.as_deref()),
        Some("Room 8")
    );
    assert_eq!(
        event
            .organizer
            .as_ref()
            .and_then(|organizer| organizer.display_name.as_deref()),
        Some("Ada")
    );
    assert_eq!(event.attendee_count, 2);
    assert_eq!(page.events[1].summary.as_deref(), Some("No attendees"));
    assert_eq!(page.events[1].attendee_count, 0);

    let serialized = serde_json::to_string(&page).expect("page json");
    assert!(!serialized.contains("evt_1"));
    assert!(!serialized.contains("evt_2"));
    assert!(!serialized.contains("private notes"));
    assert!(!serialized.contains("meeting_url"));
    assert!(!serialized.contains("app_link"));
    assert!(!serialized.contains("https://vc.example/private"));
    assert!(!serialized.contains("https://applink.example/private"));
    assert!(!serialized.contains("secret_file"));
    assert!(!serialized.contains("ou_secret"));
    assert!(!serialized.contains("u_secret"));
    assert!(!serialized.contains("address should not surface"));
    assert!(!serialized.contains("missing id"));
}

#[test]
fn get_event_success_returns_sanitized_summary_from_fixture() {
    let response = HttpResponse::new(
        200,
        include_str!("fixtures/event_get_success.json").to_string(),
    );
    let mut client = FeishuCalendarReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(response),
    );

    let event = client
        .get_event_summary(sample_event_read_request())
        .expect("success");

    assert_eq!(event.summary.as_deref(), Some("Customer review"));
    assert_eq!(
        event
            .start_time_info
            .as_ref()
            .and_then(|time| time.timestamp.as_deref()),
        Some("1779987600")
    );
    assert_eq!(
        event
            .location
            .as_ref()
            .and_then(|location| location.name.as_deref()),
        Some("Room 8")
    );
    assert_eq!(
        event
            .organizer
            .as_ref()
            .and_then(|organizer| organizer.display_name.as_deref()),
        Some("Ada")
    );
    assert_eq!(event.attendee_count, 2);

    let serialized = serde_json::to_string(&event).expect("event json");
    assert!(!serialized.contains("evt_1"));
    assert!(!serialized.contains("private notes"));
    assert!(!serialized.contains("meeting_url"));
    assert!(!serialized.contains("app_link"));
    assert!(!serialized.contains("https://vc.example/private"));
    assert!(!serialized.contains("https://applink.example/private"));
    assert!(!serialized.contains("secret_file"));
    assert!(!serialized.contains("ou_secret"));
    assert!(!serialized.contains("u_secret"));
    assert!(!serialized.contains("address should not surface"));
}

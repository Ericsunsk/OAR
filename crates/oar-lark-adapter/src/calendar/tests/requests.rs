use serde_json::json;

use super::support::{
    sample_event_read_request, sample_instance_view_request, sample_primary_request, sample_request,
};
use crate::calendar::{
    parse_calendar_event_source_ref, FeishuCalendarReadClient, FeishuCalendarReadError,
};
use crate::config::FeishuOpenApiConfig;
use crate::oauth::HttpResponse;
use crate::test_support::http::FakeHttpClient;

#[test]
fn batch_free_busy_request_contains_user_token_but_debug_redacts_it() {
    let mut client = FeishuCalendarReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({"code":0,"data":{"freebusy_lists":[]}}).to_string(),
        )),
    );

    let request = sample_request();
    let request_debug = format!("{request:?}");
    assert!(!request_debug.contains("u-very-secret-calendar-token"));
    assert!(request_debug.contains("[REDACTED]"));

    client.batch_free_busy(request).expect("success");
    let sent = client
        .http_client()
        .request
        .as_ref()
        .expect("captured request");

    assert_eq!(sent.method, "POST");
    assert!(sent
        .url
        .ends_with("/open-apis/calendar/v4/freebusy/batch?user_id_type=open_id"));
    assert_eq!(sent.body["user_ids"], json!(["ou_current_user"]));
    assert_eq!(sent.body["include_external_calendar"], json!(false));
    assert_eq!(sent.body["only_busy"], json!(true));
    assert!(sent.headers.iter().any(|(name, value)| {
        name == "Authorization" && value == "Bearer u-very-secret-calendar-token"
    }));

    let sent_debug = format!("{sent:?}");
    assert!(!sent_debug.contains("u-very-secret-calendar-token"));
    assert!(sent_debug.contains("[REDACTED]"));
}

#[test]
fn primary_calendar_request_uses_current_user_token_and_redacts_debug() {
    let mut client = FeishuCalendarReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({"code":0,"data":{"calendars":[{"calendar":{"calendar_id":"cal_primary","summary":"Primary"}}]}})
                .to_string(),
        )),
    );

    let request = sample_primary_request();
    let request_debug = format!("{request:?}");
    assert!(!request_debug.contains("u-very-secret-calendar-token"));
    assert!(request_debug.contains("[REDACTED]"));

    client.primary_calendar(request).expect("success");
    let sent = client
        .http_client()
        .request
        .as_ref()
        .expect("captured request");

    assert_eq!(sent.method, "POST");
    assert!(sent
        .url
        .ends_with("/open-apis/calendar/v4/calendars/primary"));
    assert_eq!(sent.body, json!({}));
    assert!(sent.headers.iter().any(|(name, value)| {
        name == "Authorization" && value == "Bearer u-very-secret-calendar-token"
    }));
    let sent_debug = format!("{sent:?}");
    assert!(!sent_debug.contains("u-very-secret-calendar-token"));
    assert!(sent_debug.contains("[REDACTED_PATH]"));
}

#[test]
fn instance_view_request_percent_encodes_path_and_query() {
    let mut client = FeishuCalendarReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({"code":0,"data":{"instances":[]}}).to_string(),
        )),
    );

    let request = sample_instance_view_request();
    let request_debug = format!("{request:?}");
    assert!(!request_debug.contains("u-very-secret-calendar-token"));
    assert!(!request_debug.contains("primary calendar"));
    assert!(!request_debug.contains("飞"));
    assert!(request_debug.contains("[REDACTED]"));

    client.event_instance_view(request).expect("success");
    let sent = client
        .http_client()
        .request
        .as_ref()
        .expect("captured request");

    assert_eq!(sent.method, "GET");
    assert!(sent.url.ends_with(
        "/open-apis/calendar/v4/calendars/primary%20calendar%2F%E9%A3%9E/events/instance_view?start_time=1779984000&end_time=1780070400"
    ));
    assert_eq!(sent.body, json!({}));
    assert!(sent.headers.iter().any(|(name, value)| {
        name == "Authorization" && value == "Bearer u-very-secret-calendar-token"
    }));
    let sent_debug = format!("{sent:?}");
    assert!(!sent_debug.contains("u-very-secret-calendar-token"));
    assert!(!sent_debug.contains("primary%20calendar"));
    assert!(!sent_debug.contains("%E9%A3%9E"));
    assert!(sent_debug.contains("[REDACTED_PATH]"));
}

#[test]
fn get_event_request_uses_source_ref_components_and_redacts_debug() {
    let mut client = FeishuCalendarReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({"code":0,"data":{"event":{"event_id":"evt_1"}}}).to_string(),
        )),
    );

    let mut request = sample_event_read_request();
    request.source_ref = "feishu://calendar/cal%3Aprimary/events/evt%2F1".to_string();
    let request_debug = format!("{request:?}");
    assert!(!request_debug.contains("u-very-secret-calendar-token"));
    assert!(!request_debug.contains("cal%3Aprimary"));
    assert!(!request_debug.contains("evt%2F1"));
    assert!(request_debug.contains("[REDACTED]"));

    client.get_event_summary(request).expect("success");
    let sent = client
        .http_client()
        .request
        .as_ref()
        .expect("captured request");

    assert_eq!(sent.method, "GET");
    assert!(sent
        .url
        .ends_with("/open-apis/calendar/v4/calendars/cal%3Aprimary/events/evt%2F1"));
    assert_eq!(sent.body, json!({}));
    assert!(sent.headers.iter().any(|(name, value)| {
        name == "Authorization" && value == "Bearer u-very-secret-calendar-token"
    }));
    let sent_debug = format!("{sent:?}");
    assert!(!sent_debug.contains("u-very-secret-calendar-token"));
    assert!(!sent_debug.contains("cal%3Aprimary"));
    assert!(!sent_debug.contains("evt%2F1"));
    assert!(sent_debug.contains("[REDACTED_PATH]"));

    let parsed =
        parse_calendar_event_source_ref("calendar://cal_secret/events/evt_secret").unwrap();
    let parsed_debug = format!("{parsed:?}");
    assert!(!parsed_debug.contains("cal_secret"));
    assert!(!parsed_debug.contains("evt_secret"));
}

#[test]
fn batch_free_busy_rejects_unsafe_request_shapes_before_http() {
    let response = HttpResponse::new(
        200,
        json!({"code":0,"data":{"freebusy_lists":[]}}).to_string(),
    );
    let mut client = FeishuCalendarReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(response),
    );

    let mut request = sample_request();
    request.user_ids = vec![];
    assert_eq!(
        client.batch_free_busy(request),
        Err(FeishuCalendarReadError::InvalidRequest)
    );

    let mut request = sample_request();
    request.user_ids = vec!["ou_current_user/unsafe".to_string()];
    assert_eq!(
        client.batch_free_busy(request),
        Err(FeishuCalendarReadError::InvalidRequest)
    );

    let mut request = sample_request();
    request.time_min = "not a time".to_string();
    assert_eq!(
        client.batch_free_busy(request),
        Err(FeishuCalendarReadError::InvalidRequest)
    );
}

#[test]
fn instance_view_rejects_invalid_request_shapes_before_http() {
    let response = HttpResponse::new(200, json!({"code":0,"data":{"instances":[]}}).to_string());
    let mut client = FeishuCalendarReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(response),
    );

    let mut request = sample_instance_view_request();
    request.calendar_id = " \t".to_string();
    assert_eq!(
        client.event_instance_view(request),
        Err(FeishuCalendarReadError::InvalidRequest)
    );

    let mut request = sample_instance_view_request();
    request.end_time = request.start_time;
    assert_eq!(
        client.event_instance_view(request),
        Err(FeishuCalendarReadError::InvalidRequest)
    );

    let mut request = sample_instance_view_request();
    request.end_time = request.start_time + 40 * 24 * 60 * 60;
    assert_eq!(
        client.event_instance_view(request),
        Err(FeishuCalendarReadError::InvalidRequest)
    );
}

#[test]
fn get_event_rejects_invalid_source_ref_before_http() {
    let response = HttpResponse::new(
        200,
        json!({"code":0,"data":{"event":{"event_id":"evt_1"}}}).to_string(),
    );
    let mut client = FeishuCalendarReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(response),
    );

    let mut request = sample_event_read_request();
    request.source_ref = "calendar://cal_1/events/evt/1".to_string();

    assert_eq!(
        client.get_event_summary(request),
        Err(FeishuCalendarReadError::InvalidSourceRef)
    );
    assert!(client.http_client().request.is_none());
}

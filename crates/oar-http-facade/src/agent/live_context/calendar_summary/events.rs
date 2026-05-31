use std::time::{Duration, SystemTime, UNIX_EPOCH};

use oar_lark_adapter::{
    AsyncFeishuCalendarRead, CalendarEventInstance, CalendarEventInstancePage,
    CalendarEventInstanceViewRequest, CalendarEventTimeInfo, CalendarPrimaryRequest,
    FeishuCalendarReadClient, ReqwestAsyncHttpClient, SecretString,
};

use super::super::summary::{
    compact_text, evidence_label, examples_suffix, finalize_summary, tool_live_label,
    truncate_chars,
};
use super::{lookahead_window_text, CALENDAR_LOOKAHEAD_DAYS};
use crate::agent::request::AgentEvidenceRefDTO;
use crate::agent::tools::AgentReadTool;
use crate::feishu_auth::iso8601_utc;

const EVENT_EXAMPLE_LIMIT: usize = 5;

pub(in crate::agent::live_context) async fn read_my_calendar_events_summary(
    calendar_client: &mut FeishuCalendarReadClient<ReqwestAsyncHttpClient>,
    access_token: SecretString,
    now: SystemTime,
) -> Result<String, oar_lark_adapter::FeishuCalendarReadError> {
    let primary_page = calendar_client
        .primary_calendar(CalendarPrimaryRequest {
            user_access_token: access_token.clone(),
        })
        .await?;
    let calendar_id = primary_page.calendar.calendar_id;
    let time_min = unix_seconds(now);
    let time_max = unix_seconds(now + Duration::from_secs(CALENDAR_LOOKAHEAD_DAYS * 24 * 60 * 60));
    let page = calendar_client
        .event_instance_view(CalendarEventInstanceViewRequest {
            user_access_token: access_token,
            calendar_id,
            start_time: time_min,
            end_time: time_max,
        })
        .await?;

    Ok(summarize_event_instances_page(&page))
}

fn summarize_event_instances_page(page: &CalendarEventInstancePage) -> String {
    let tool_label = tool_live_label(AgentReadTool::CalendarEvents);
    if page.events.is_empty() {
        return format!(
            "{tool_label}｜实时：{}未读取到日程实例。",
            lookahead_window_text()
        );
    }

    let examples = page
        .events
        .iter()
        .map(summarize_calendar_event)
        .take(EVENT_EXAMPLE_LIMIT)
        .collect::<Vec<_>>();
    let suffix = examples_suffix(&examples);

    finalize_summary(format!(
        "{tool_label}｜实时：{}读取到 {} 条日程实例{}。",
        lookahead_window_text(),
        page.events.len(),
        suffix
    ))
}

fn summarize_calendar_event(event: &CalendarEventInstance) -> String {
    let start = event_time_text(event.start_time_info.as_ref());
    let end = event_time_text(event.end_time_info.as_ref());
    let title =
        compact_optional_text(event.summary.as_deref()).unwrap_or_else(|| "未命名日程".to_string());
    let location = event
        .location
        .as_ref()
        .and_then(|location| compact_optional_text(location.name.as_deref()));
    let organizer = event
        .organizer
        .as_ref()
        .and_then(|organizer| compact_optional_text(organizer.display_name.as_deref()));
    let status = compact_optional_text(event.status.as_deref());
    let free_busy = compact_optional_text(event.free_busy_status.as_deref());

    let mut parts = Vec::new();
    if let (Some(start), Some(end)) = (start, end) {
        parts.push(format!(
            "{}-{}",
            truncate_chars(&start, 20),
            truncate_chars(&end, 20)
        ));
    }
    parts.push(format!("「{}」", truncate_chars(&title, 28)));
    if let Some(location) = location {
        parts.push(format!("地点 {}", truncate_chars(&location, 18)));
    }
    if let Some(organizer) = organizer {
        parts.push(format!("组织者 {}", truncate_chars(&organizer, 18)));
    }
    if let Some(status) = status {
        parts.push(format!("状态 {}", truncate_chars(&status, 14)));
    }
    if let Some(free_busy) = free_busy {
        parts.push(format!("忙闲 {}", truncate_chars(&free_busy, 14)));
    }
    if event.attendee_count > 0 {
        parts.push(format!("参与人 {} 位", event.attendee_count));
    }

    parts.join("，")
}

pub(in crate::agent::live_context) fn build_calendar_event_live_summary(
    evidence_ref: &AgentEvidenceRefDTO,
    event: &CalendarEventInstance,
) -> String {
    let label = evidence_label(evidence_ref);
    let event_summary = summarize_calendar_event(event);

    finalize_summary(format!("{label}｜实时：日程：{event_summary}。"))
}

fn event_time_text(time_info: Option<&CalendarEventTimeInfo>) -> Option<String> {
    time_info.and_then(|time_info| {
        time_info
            .timestamp
            .as_deref()
            .and_then(iso8601_utc_from_unix_seconds)
            .or_else(|| compact_optional_text(time_info.date.as_deref()))
            .or_else(|| compact_optional_text(time_info.timestamp.as_deref()))
    })
}

fn iso8601_utc_from_unix_seconds(value: &str) -> Option<String> {
    let seconds = value.trim().parse::<u64>().ok()?;
    Some(compact_iso8601_utc_minute(&iso8601_utc(
        UNIX_EPOCH + Duration::from_secs(seconds),
    )))
}

fn compact_iso8601_utc_minute(value: &str) -> String {
    if value.len() >= 17 && value.ends_with('Z') {
        format!("{}Z", &value[..16])
    } else {
        value.to_string()
    }
}

fn compact_optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(compact_text)
        .filter(|value| !value.is_empty())
        .map(|value| truncate_chars(&value, 80))
}

fn unix_seconds(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use oar_lark_adapter::{CalendarEventLocation, CalendarEventOrganizer};

    #[test]
    fn event_summary_is_counted_limited_and_sanitized() {
        let page = CalendarEventInstancePage {
            events: vec![
                CalendarEventInstance {
                    summary: Some(" Team sync ".to_string()),
                    start_time_info: Some(CalendarEventTimeInfo {
                        timestamp: Some("1780000000".to_string()),
                        timezone: Some("Asia/Shanghai".to_string()),
                        date: None,
                    }),
                    end_time_info: Some(CalendarEventTimeInfo {
                        timestamp: Some("1780003600".to_string()),
                        timezone: Some("Asia/Shanghai".to_string()),
                        date: None,
                    }),
                    status: Some("confirmed".to_string()),
                    visibility: Some("default".to_string()),
                    free_busy_status: Some("busy".to_string()),
                    location: Some(CalendarEventLocation {
                        name: Some(" Boardroom ".to_string()),
                    }),
                    organizer: Some(CalendarEventOrganizer {
                        display_name: Some(" Alice ".to_string()),
                    }),
                    attendee_count: 2,
                },
                event_with_title("Second"),
                event_with_title("Third"),
                event_with_title("Fourth"),
                event_with_title("Fifth"),
                event_with_title("Sixth"),
            ],
        };

        let summary = summarize_event_instances_page(&page);

        assert!(summary.contains("未来 7 天读取到 6 条日程实例"));
        assert!(summary.contains("；示例："));
        assert!(summary.contains("2026-05-28T20:26Z-2026-05-28T21:26Z"));
        assert!(summary.contains("Team sync"));
        assert!(summary.contains("地点 Boardroom"));
        assert!(summary.contains("组织者 Alice"));
        assert!(summary.contains("状态 confirmed"));
        assert!(summary.contains("忙闲 busy"));
        assert!(summary.contains("参与人 2 位"));
        assert!(summary.contains("Second"));
        assert!(!summary.contains("1780000000"));
        assert!(!summary.contains("Sixth"));
        assert!(!summary.contains("evt_secret"));
    }

    #[test]
    fn empty_event_summary_is_clear() {
        let page = CalendarEventInstancePage { events: vec![] };

        assert_eq!(
            summarize_event_instances_page(&page),
            format!(
                "{}｜实时：未来 7 天未读取到日程实例。",
                tool_live_label(AgentReadTool::CalendarEvents)
            )
        );
    }

    #[test]
    fn calendar_event_live_summary_is_sanitized_and_compact() {
        let evidence_ref = AgentEvidenceRefDTO {
            source_type: "calendar".to_string(),
            source_ref: "calendar://cal_secret/events/evt_secret".to_string(),
            summary: "客户会议证据".to_string(),
        };
        let summary = build_calendar_event_live_summary(
            &evidence_ref,
            &CalendarEventInstance {
                summary: Some(" Customer review ".to_string()),
                start_time_info: Some(CalendarEventTimeInfo {
                    timestamp: Some("1780000000".to_string()),
                    timezone: Some("Asia/Shanghai".to_string()),
                    date: None,
                }),
                end_time_info: None,
                status: Some("confirmed".to_string()),
                visibility: Some("default".to_string()),
                free_busy_status: Some("busy".to_string()),
                location: Some(CalendarEventLocation {
                    name: Some(" Boardroom ".to_string()),
                }),
                organizer: Some(CalendarEventOrganizer {
                    display_name: Some(" Alice ".to_string()),
                }),
                attendee_count: 3,
            },
        );

        assert!(summary.contains("客户会议证据｜实时：日程："));
        assert!(summary.contains("Customer review"));
        assert!(summary.contains("地点 Boardroom"));
        assert!(summary.contains("组织者 Alice"));
        assert!(summary.contains("参与人 3 位"));
        assert!(!summary.contains("cal_secret"));
        assert!(!summary.contains("evt_secret"));
        assert!(!summary.contains("1780000000"));
    }

    fn event_with_title(title: &str) -> CalendarEventInstance {
        CalendarEventInstance {
            summary: Some(title.to_string()),
            start_time_info: None,
            end_time_info: None,
            status: None,
            visibility: None,
            free_busy_status: None,
            location: None,
            organizer: None,
            attendee_count: 0,
        }
    }
}

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use oar_lark_adapter::{
    AsyncFeishuCalendarRead, CalendarEventInstance, CalendarEventInstancePage,
    CalendarEventInstanceViewRequest, CalendarEventTimeInfo, CalendarFreeBusyBatchRequest,
    CalendarFreeBusyPage, CalendarPrimaryRequest, CalendarUserIdType, FeishuCalendarReadClient,
    ReqwestAsyncHttpClient, SecretString,
};

use super::summary::{compact_text, finalize_summary, truncate_chars};
use crate::feishu_auth::iso8601_utc;

const FREE_BUSY_WINDOW_DAYS: u64 = 7;
const BUSY_SLOT_EXAMPLE_LIMIT: usize = 4;
const EVENT_EXAMPLE_LIMIT: usize = 5;

pub(super) async fn read_my_calendar_free_busy_summary(
    calendar_client: &mut FeishuCalendarReadClient<ReqwestAsyncHttpClient>,
    access_token: SecretString,
    lark_open_id: &str,
    now: SystemTime,
) -> Result<String, oar_lark_adapter::FeishuCalendarReadError> {
    let time_min = iso8601_utc(now);
    let time_max = iso8601_utc(now + Duration::from_secs(FREE_BUSY_WINDOW_DAYS * 24 * 60 * 60));
    let page = calendar_client
        .batch_free_busy(CalendarFreeBusyBatchRequest {
            user_access_token: access_token,
            user_ids: vec![lark_open_id.to_string()],
            time_min,
            time_max,
            include_external_calendar: false,
            only_busy: true,
            need_rsvp_status: false,
            user_id_type: CalendarUserIdType::OpenId,
        })
        .await?;

    Ok(summarize_free_busy_page(&page))
}

pub(super) async fn read_my_calendar_events_summary(
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
    let time_max = unix_seconds(now + Duration::from_secs(FREE_BUSY_WINDOW_DAYS * 24 * 60 * 60));
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

fn summarize_free_busy_page(page: &CalendarFreeBusyPage) -> String {
    let busy_items = page
        .lists
        .iter()
        .flat_map(|list| list.busy_items.iter())
        .collect::<Vec<_>>();

    if busy_items.is_empty() {
        return "工具 feishu.calendar.summarize_my_free_busy｜实时：未来 7 天未读取到忙碌时段。"
            .to_string();
    }

    let examples = busy_items
        .iter()
        .filter_map(|item| {
            let start = item
                .start_time
                .as_deref()
                .map(compact_text)
                .filter(|value| !value.is_empty())?;
            let end = item
                .end_time
                .as_deref()
                .map(compact_text)
                .filter(|value| !value.is_empty())?;
            Some(format!(
                "{}-{}",
                truncate_chars(&start, 20),
                truncate_chars(&end, 20)
            ))
        })
        .take(BUSY_SLOT_EXAMPLE_LIMIT)
        .collect::<Vec<_>>();
    let examples_suffix = if examples.is_empty() {
        String::new()
    } else {
        format!("；示例：{}", examples.join(" / "))
    };

    finalize_summary(format!(
        "工具 feishu.calendar.summarize_my_free_busy｜实时：未来 7 天读取到 {} 段忙碌时段{}。",
        busy_items.len(),
        examples_suffix
    ))
}

fn summarize_event_instances_page(page: &CalendarEventInstancePage) -> String {
    if page.events.is_empty() {
        return "工具 feishu.calendar.summarize_my_events｜实时：未来 7 天未读取到日程实例。"
            .to_string();
    }

    let examples = page
        .events
        .iter()
        .map(summarize_event_example)
        .take(EVENT_EXAMPLE_LIMIT)
        .collect::<Vec<_>>();
    let examples_suffix = if examples.is_empty() {
        String::new()
    } else {
        format!("；示例：{}", examples.join(" / "))
    };

    finalize_summary(format!(
        "工具 feishu.calendar.summarize_my_events｜实时：未来 7 天读取到 {} 条日程实例{}。",
        page.events.len(),
        examples_suffix
    ))
}

fn summarize_event_example(event: &CalendarEventInstance) -> String {
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

fn event_time_text(time_info: Option<&CalendarEventTimeInfo>) -> Option<String> {
    time_info.and_then(|time_info| {
        compact_optional_text(time_info.timestamp.as_deref().or(time_info.date.as_deref()))
    })
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
    use oar_lark_adapter::{
        CalendarEventLocation, CalendarEventOrganizer, CalendarFreeBusyItem, CalendarFreeBusyList,
    };

    #[test]
    fn free_busy_summary_is_counted_and_sanitized() {
        let page = CalendarFreeBusyPage {
            lists: vec![CalendarFreeBusyList {
                busy_items: vec![CalendarFreeBusyItem {
                    start_time: Some("2026-05-29T10:00:00+08:00".to_string()),
                    end_time: Some("2026-05-29T11:00:00+08:00".to_string()),
                }],
            }],
        };

        let summary = summarize_free_busy_page(&page);

        assert!(summary.contains("未来 7 天读取到 1 段忙碌时段"));
        assert!(!summary.contains("accepted"));
        assert!(!summary.contains("ou_sensitive"));
    }

    #[test]
    fn empty_free_busy_summary_is_clear() {
        let page = CalendarFreeBusyPage { lists: vec![] };

        assert_eq!(
            summarize_free_busy_page(&page),
            "工具 feishu.calendar.summarize_my_free_busy｜实时：未来 7 天未读取到忙碌时段。"
        );
    }

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
        assert!(summary.contains("1780000000-1780003600"));
        assert!(summary.contains("Team sync"));
        assert!(summary.contains("地点 Boardroom"));
        assert!(summary.contains("组织者 Alice"));
        assert!(summary.contains("状态 confirmed"));
        assert!(summary.contains("忙闲 busy"));
        assert!(summary.contains("参与人 2 位"));
        assert!(summary.contains("Fifth"));
        assert!(!summary.contains("Sixth"));
        assert!(!summary.contains("evt_secret"));
    }

    #[test]
    fn empty_event_summary_is_clear() {
        let page = CalendarEventInstancePage { events: vec![] };

        assert_eq!(
            summarize_event_instances_page(&page),
            "工具 feishu.calendar.summarize_my_events｜实时：未来 7 天未读取到日程实例。"
        );
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

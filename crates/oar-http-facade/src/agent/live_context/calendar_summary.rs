use std::time::{Duration, SystemTime};

use oar_lark_adapter::{
    AsyncFeishuCalendarRead, CalendarFreeBusyBatchRequest, CalendarFreeBusyPage,
    CalendarUserIdType, FeishuCalendarReadClient, ReqwestAsyncHttpClient, SecretString,
};

use super::summary::{compact_text, finalize_summary, truncate_chars};
use crate::feishu_auth::iso8601_utc;

const FREE_BUSY_WINDOW_DAYS: u64 = 7;
const BUSY_SLOT_EXAMPLE_LIMIT: usize = 4;

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

#[cfg(test)]
mod tests {
    use super::*;
    use oar_lark_adapter::{CalendarFreeBusyItem, CalendarFreeBusyList};

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
}

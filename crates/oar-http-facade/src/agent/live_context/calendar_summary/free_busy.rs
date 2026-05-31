use std::time::{Duration, SystemTime};

use oar_lark_adapter::{
    AsyncFeishuCalendarRead, CalendarFreeBusyBatchRequest, CalendarFreeBusyPage,
    CalendarUserIdType, FeishuCalendarReadClient, ReqwestAsyncHttpClient, SecretString,
};

use super::super::summary::{
    compact_text, examples_suffix, finalize_summary, tool_live_label, truncate_chars,
};
use super::{lookahead_window_text, CALENDAR_LOOKAHEAD_DAYS};
use crate::agent::tools::AgentReadTool;
use crate::feishu_auth::iso8601_utc;

const BUSY_SLOT_EXAMPLE_LIMIT: usize = 4;

pub(in crate::agent::live_context) async fn read_my_calendar_free_busy_summary(
    calendar_client: &mut FeishuCalendarReadClient<ReqwestAsyncHttpClient>,
    access_token: SecretString,
    lark_open_id: &str,
    now: SystemTime,
) -> Result<String, oar_lark_adapter::FeishuCalendarReadError> {
    let time_min = iso8601_utc(now);
    let time_max = iso8601_utc(now + Duration::from_secs(CALENDAR_LOOKAHEAD_DAYS * 24 * 60 * 60));
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
    let tool_label = tool_live_label(AgentReadTool::CalendarFreeBusy);
    let busy_items = page
        .lists
        .iter()
        .flat_map(|list| list.busy_items.iter())
        .collect::<Vec<_>>();

    if busy_items.is_empty() {
        return format!(
            "{tool_label}｜实时：{}未读取到忙碌时段。",
            lookahead_window_text()
        );
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
    let suffix = examples_suffix(&examples);

    finalize_summary(format!(
        "{tool_label}｜实时：{}读取到 {} 段忙碌时段{}。",
        lookahead_window_text(),
        busy_items.len(),
        suffix
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
        assert!(summary.contains("；示例：2026-05-29T10:00"));
        assert!(summary.contains("-2026-05-29T11:00"));
        assert!(!summary.contains("accepted"));
        assert!(!summary.contains("ou_sensitive"));
    }

    #[test]
    fn empty_free_busy_summary_is_clear() {
        let page = CalendarFreeBusyPage { lists: vec![] };

        assert_eq!(
            summarize_free_busy_page(&page),
            format!(
                "{}｜实时：未来 7 天未读取到忙碌时段。",
                tool_live_label(AgentReadTool::CalendarFreeBusy)
            )
        );
    }
}

use oar_core::security::contains_sensitive_marker;
use oar_lark_adapter::{
    AsyncFeishuMinutesRead, FeishuMinuteSearchRequest, FeishuMinutesReadClient,
    FeishuMinutesReadError, MinuteReadSummary, ReqwestAsyncHttpClient, SecretString,
};

use super::summary::{
    compact_text, examples_suffix, finalize_summary, format_minutes_duration_ms, tool_live_label,
    truncate_chars,
};
use crate::agent::tools::AgentReadTool;

const MY_MINUTES_EXAMPLE_LIMIT: usize = 5;
const MY_MINUTES_PAGE_SIZE: u16 = 30;
const MY_MINUTES_PAGE_LIMIT: usize = 2;

pub(super) async fn read_my_minutes_summary(
    minutes_client: &mut FeishuMinutesReadClient<ReqwestAsyncHttpClient>,
    access_token: SecretString,
    owner_open_id: &str,
) -> Result<String, FeishuMinutesReadError> {
    let mut minutes = Vec::new();
    let mut page_token = None;
    let mut has_more = false;
    let mut total = None;

    for _ in 0..MY_MINUTES_PAGE_LIMIT {
        let page = minutes_client
            .search_minute_summaries(FeishuMinuteSearchRequest {
                user_access_token: access_token.clone(),
                page_size: Some(MY_MINUTES_PAGE_SIZE),
                page_token,
                query: None,
                owner_ids: vec![owner_open_id.to_string()],
                participant_ids: vec![],
            })
            .await?;
        if total.is_none() {
            total = page.total;
        }
        minutes.extend(page.minutes);
        has_more = page.has_more;
        page_token = page.page_token;
        if !has_more || page_token.is_none() {
            break;
        }
    }

    Ok(build_my_minutes_summary(&minutes, total, has_more))
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn build_my_minutes_summary(
    minutes: &[MinuteReadSummary],
    total: Option<u64>,
    has_more: bool,
) -> String {
    let tool_label = tool_live_label(AgentReadTool::MinutesSummary);
    if minutes.is_empty() {
        return format!("{tool_label}｜实时：未读取到当前用户的妙记。");
    }
    let count = total.unwrap_or(minutes.len() as u64);

    let examples = minutes
        .iter()
        .map(minute_example)
        .take(MY_MINUTES_EXAMPLE_LIMIT)
        .collect::<Vec<_>>();
    let examples_text = examples_suffix(&examples);
    let more_suffix = if has_more {
        "；仍可能有更多妙记"
    } else {
        ""
    };

    finalize_summary(format!(
        "{tool_label}｜实时：读取到 {} 条当前用户妙记{}{}。",
        count, examples_text, more_suffix
    ))
}

#[cfg_attr(not(test), allow(dead_code))]
fn minute_example(minute: &MinuteReadSummary) -> String {
    let title = minute
        .title
        .as_deref()
        .map(compact_text)
        .filter(|value| !value.is_empty())
        .filter(|value| !contains_sensitive_marker(value))
        .unwrap_or_else(|| "未命名妙记".to_string());
    let duration = minute
        .duration_ms
        .as_deref()
        .and_then(|value| value.parse::<u64>().ok())
        .map(format_minutes_duration_ms)
        .map(|value| format!("，时长 {value}"))
        .unwrap_or_default();
    let create_time = minute
        .create_time_ms
        .as_deref()
        .filter(|value| value.chars().all(|ch| ch.is_ascii_digit()))
        .map(|value| format!("，创建时间戳 {}", truncate_chars(value, 18)))
        .unwrap_or_default();

    format!(
        "「{}」{}{}",
        truncate_chars(&title, 28),
        duration,
        create_time
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn my_minutes_summary_is_bounded_and_sanitized() {
        let minutes = vec![
            minute(" Weekly Sync ", Some("314000"), Some("1669098360477")),
            minute("access_token review", Some("12000"), Some("1669098360478")),
            minute("Second", None, None),
            minute("Third", None, None),
            minute("Fourth", None, None),
            minute("Fifth", None, None),
        ];

        let summary = build_my_minutes_summary(&minutes, Some(42), true);

        assert!(summary.contains("读取到 42 条当前用户妙记"));
        assert!(summary.contains("Weekly Sync"));
        assert!(summary.contains("时长 5分14秒"));
        assert!(summary.contains("创建时间戳 1669098360477"));
        assert!(summary.contains("未命名妙记"));
        assert!(summary.contains("仍可能有更多妙记"));
        assert!(!summary.contains("access_token"));
        assert!(!summary.contains("Fifth"));
    }

    #[test]
    fn my_minutes_summary_handles_empty_result() {
        assert_eq!(
            build_my_minutes_summary(&[], Some(0), false),
            format!(
                "{}｜实时：未读取到当前用户的妙记。",
                tool_live_label(AgentReadTool::MinutesSummary)
            )
        );
    }

    fn minute(
        title: &str,
        duration_ms: Option<&str>,
        create_time_ms: Option<&str>,
    ) -> MinuteReadSummary {
        MinuteReadSummary {
            title: Some(title.to_string()),
            duration_ms: duration_ms.map(str::to_string),
            create_time_ms: create_time_ms.map(str::to_string),
        }
    }
}

use crate::agent::request::AgentStreamRequest;

use super::common::{
    asks_to_count, asks_to_read, contains_latin_token, is_self_scoped, latest_user_text,
    mentions_feishu, targets_non_self,
};

pub(super) fn latest_user_requests_feishu_okr_summary(request: &AgentStreamRequest) -> bool {
    let Some(latest_user_text) = latest_user_text(request) else {
        return false;
    };

    latest_user_has_explicit_self_okr_read_intent(latest_user_text)
        || (latest_user_has_contextual_feishu_count_intent(latest_user_text)
            && request_has_recent_okr_topic(request))
}

fn latest_user_has_explicit_self_okr_read_intent(text: &str) -> bool {
    mentions_okr(text)
        && asks_to_read(text)
        && is_self_scoped(text)
        && !targets_non_self(text)
        && !mentions_non_okr_goal_context(text)
}

fn latest_user_has_contextual_feishu_count_intent(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    mentions_feishu(text)
        && is_self_scoped(text)
        && !targets_non_self(text)
        && !mentions_non_okr_feishu_domain(text)
        && (asks_to_count(text)
            || contains_latin_token(&normalized, "current")
            || text.contains("目前"))
}

fn request_has_recent_okr_topic(request: &AgentStreamRequest) -> bool {
    request
        .recent_messages()
        .any(|message| mentions_okr(&message.text))
        || mentions_okr(&request.context.title)
        || mentions_okr(&request.context.risk_reason)
        || mentions_okr(&request.context.action_summary)
        || mentions_okr(&request.context.workspace_summary)
        || request
            .context
            .workspace_signals
            .iter()
            .any(|value| mentions_okr(value))
        || request
            .context
            .evidence_summaries
            .iter()
            .any(|value| mentions_okr(value))
        || request.context.evidence_refs.iter().any(|evidence_ref| {
            mentions_okr(&evidence_ref.source_type)
                || mentions_okr(&evidence_ref.source_ref)
                || mentions_okr(&evidence_ref.summary)
        })
}

fn mentions_okr(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    contains_latin_token(&normalized, "okr")
        || contains_latin_token(&normalized, "okrs")
        || contains_latin_token(&normalized, "kr")
        || contains_latin_token(&normalized, "krs")
        || text.contains("关键结果")
        || text.contains("飞书 OKR")
        || text.contains("飞书okr")
        || text.contains("飞书目标")
}

fn mentions_non_okr_goal_context(text: &str) -> bool {
    text.contains("目标客户") || text.contains("客户目标")
}

fn mentions_non_okr_feishu_domain(text: &str) -> bool {
    mentions_non_okr_goal_context(text)
        || text.contains("消息")
        || text.contains("聊天")
        || text.contains("会话")
        || text.contains("任务")
        || text.contains("日历")
        || text.contains("日程")
        || text.contains("忙闲")
        || text.contains("空闲")
        || text.contains("会议")
        || text.contains("文档")
        || text.contains("审批")
        || text.contains("邮件")
}

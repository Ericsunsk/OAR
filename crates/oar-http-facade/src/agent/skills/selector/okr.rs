use crate::agent::request::AgentStreamRequest;
use crate::agent::tools::AgentReadTool;

use super::common::{
    asks_to_count, asks_to_read, asks_to_run_read_tool, contains_latin_token, is_self_scoped,
    latest_user_text, mentions_feishu, targets_non_self,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::agent) enum FeishuOkrReadIntent {
    Summary,
    Progress,
}

pub(in crate::agent) fn latest_user_feishu_okr_read_intents(
    request: &AgentStreamRequest,
) -> Vec<FeishuOkrReadIntent> {
    let Some(latest_user_text) = latest_user_text(request) else {
        return Vec::new();
    };

    if asks_okr_write(latest_user_text) {
        return Vec::new();
    }

    let mut intents = Vec::new();
    if latest_user_requests_okr_read_tool(latest_user_text, AgentReadTool::OkrSummary)
        || latest_user_has_explicit_self_okr_summary_intent(latest_user_text)
        || (latest_user_has_contextual_feishu_count_intent(latest_user_text)
            && request_has_recent_okr_topic(request))
    {
        intents.push(FeishuOkrReadIntent::Summary);
    }
    if latest_user_requests_okr_read_tool(latest_user_text, AgentReadTool::OkrProgress)
        || latest_user_has_explicit_self_okr_progress_intent(latest_user_text)
    {
        intents.push(FeishuOkrReadIntent::Progress);
    }

    intents
}

fn latest_user_requests_okr_read_tool(text: &str, tool: AgentReadTool) -> bool {
    asks_to_run_read_tool(text, tool.spec().name) && !targets_non_self(text)
}

fn latest_user_has_explicit_self_okr_summary_intent(text: &str) -> bool {
    let asks_for_summary = asks_to_count(text) || mentions_summary_content(text);
    mentions_okr(text)
        && asks_to_read(text)
        && is_self_scoped(text)
        && !targets_non_self(text)
        && !mentions_non_okr_goal_context(text)
        && (asks_for_summary || !mentions_progress_context(text))
}

fn latest_user_has_explicit_self_okr_progress_intent(text: &str) -> bool {
    mentions_okr(text)
        && is_self_scoped(text)
        && !targets_non_self(text)
        && !mentions_non_okr_goal_context(text)
        && mentions_progress_context(text)
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

fn mentions_summary_content(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    text.contains("内容") || text.contains("有没有") || contains_latin_token(&normalized, "content")
}

fn mentions_progress_context(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    text.contains("进展")
        || text.contains("进度")
        || text.contains("更新记录")
        || text.contains("最近更新")
        || text.contains("上次更新")
        || text.contains("风险")
        || text.contains("延期")
        || contains_latin_token(&normalized, "progress")
        || contains_latin_token(&normalized, "updates")
        || mentions_okr_english_update_read(&normalized)
        || contains_latin_token(&normalized, "stale")
        || contains_latin_token(&normalized, "risk")
}

fn asks_okr_write(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    text.contains("新增")
        || text.contains("创建")
        || text.contains("新建")
        || text.contains("删除")
        || text.contains("提交")
        || text.contains("发布")
        || text.contains("评论")
        || text.contains("提醒")
        || text.contains("修改")
        || text.contains("设置")
        || asks_okr_chinese_update_write(text)
        || (contains_latin_token(&normalized, "update")
            && !mentions_okr_english_update_read(&normalized))
        || contains_latin_token(&normalized, "set")
        || contains_latin_token(&normalized, "write")
        || contains_latin_token(&normalized, "delete")
        || contains_latin_token(&normalized, "submit")
        || contains_latin_token(&normalized, "post")
        || contains_latin_token(&normalized, "comment")
        || contains_latin_token(&normalized, "remind")
}

fn asks_okr_chinese_update_write(text: &str) -> bool {
    text.contains("更新")
        && !text.contains("更新记录")
        && !text.contains("最近更新")
        && !text.contains("上次更新")
}

fn mentions_okr_english_update_read(normalized: &str) -> bool {
    normalized.contains("update record")
        || normalized.contains("update records")
        || normalized.contains("latest update")
        || normalized.contains("latest updates")
        || normalized.contains("recent update")
        || normalized.contains("recent updates")
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

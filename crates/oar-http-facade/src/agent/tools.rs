use oar_core::action::capability::FeishuScope;

use super::request::AgentStreamRequest;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AgentReadTool {
    FeishuOkrSummarizeMyOkr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AgentToolSpec {
    pub(super) name: &'static str,
    pub(super) required_scopes: &'static [FeishuScope],
    pub(super) effect: AgentToolEffect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AgentToolEffect {
    Read,
}

const FEISHU_OKR_SUMMARIZE_MY_OKR_SCOPES: &[FeishuScope] =
    &[FeishuScope::OkrPeriodRead, FeishuScope::OkrContentRead];

impl AgentReadTool {
    pub(super) const fn spec(self) -> AgentToolSpec {
        match self {
            Self::FeishuOkrSummarizeMyOkr => AgentToolSpec {
                name: "feishu.okr.summarize_my_okr",
                required_scopes: FEISHU_OKR_SUMMARIZE_MY_OKR_SCOPES,
                effect: AgentToolEffect::Read,
            },
        }
    }
}

pub(super) fn plan_read_tools(request: &AgentStreamRequest) -> Vec<AgentReadTool> {
    let Some(latest_user_text) = request
        .recent_messages()
        .filter(|message| message.role == "user")
        .filter_map(|message| {
            let text = message.text.trim();
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        })
        .last()
    else {
        return vec![];
    };

    if asks_for_my_okr(latest_user_text) {
        return vec![AgentReadTool::FeishuOkrSummarizeMyOkr];
    }
    vec![]
}

fn asks_for_my_okr(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    let mentions_okr = normalized.contains("okr")
        || text.contains("目标")
        || text.contains("关键结果")
        || text.contains("飞书 OKR")
        || text.contains("飞书okr");
    if !mentions_okr {
        return false;
    }

    let asks_to_read = text.contains("查")
        || text.contains("看")
        || text.contains("读")
        || text.contains("有没有")
        || text.contains("是否")
        || normalized.contains("show")
        || normalized.contains("list")
        || normalized.contains("read");
    let self_scoped = text.contains("我")
        || text.contains("我的")
        || text.contains("本人")
        || normalized.contains("my");
    asks_to_read && self_scoped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::request::{AgentConversationContextDTO, AgentMessageDTO};

    #[test]
    fn planner_requests_my_okr_summary_for_explicit_user_okr_read() {
        let request = request_with_latest_user_text("查下我的飞书 OKR 有没有内容");

        assert_eq!(
            plan_read_tools(&request),
            vec![AgentReadTool::FeishuOkrSummarizeMyOkr]
        );
        let spec = AgentReadTool::FeishuOkrSummarizeMyOkr.spec();
        assert_eq!(spec.name, "feishu.okr.summarize_my_okr");
        assert_eq!(
            spec.required_scopes,
            &[FeishuScope::OkrPeriodRead, FeishuScope::OkrContentRead]
        );
        assert_eq!(spec.effect, AgentToolEffect::Read);
    }

    #[test]
    fn planner_ignores_non_okr_or_non_self_scoped_questions() {
        assert!(plan_read_tools(&request_with_latest_user_text("解释这个风险")).is_empty());
        assert!(plan_read_tools(&request_with_latest_user_text("查团队 OKR")).is_empty());
    }

    fn request_with_latest_user_text(text: &str) -> AgentStreamRequest {
        AgentStreamRequest {
            messages: vec![AgentMessageDTO {
                role: "user".to_string(),
                text: text.to_string(),
            }],
            context: AgentConversationContextDTO {
                title: "未选择风险".to_string(),
                risk_reason: "暂无风险说明。".to_string(),
                action_summary: "暂无建议动作。".to_string(),
                evidence_summaries: vec![],
                evidence_refs: vec![],
                workspace_summary: "暂无工作区摘要。".to_string(),
                workspace_signals: vec![],
                pending_action_summaries: vec![],
                live_feishu_read_summaries: vec![],
            },
        }
    }
}

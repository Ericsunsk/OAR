use crate::agent::request::{AgentConversationContextDTO, AgentMessageDTO, AgentStreamRequest};

pub(super) fn request_with_latest_user_text(text: &str) -> AgentStreamRequest {
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
            activated_skill_summaries: vec![],
        },
    }
}

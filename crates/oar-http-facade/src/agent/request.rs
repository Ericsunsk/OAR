use serde::Deserialize;

use super::AgentRequestError;

const AGENT_CONTEXT_MESSAGE_LIMIT: usize = 12;

pub(crate) fn decode_agent_stream_request(
    body: &[u8],
) -> Result<AgentStreamRequest, AgentRequestError> {
    serde_json::from_slice(body).map_err(|_| AgentRequestError::InvalidJson)
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AgentStreamRequest {
    pub(super) messages: Vec<AgentMessageDTO>,
    pub(super) context: AgentConversationContextDTO,
}

impl AgentStreamRequest {
    pub(super) fn recent_messages(&self) -> impl Iterator<Item = &AgentMessageDTO> {
        let message_start = self
            .messages
            .len()
            .saturating_sub(AGENT_CONTEXT_MESSAGE_LIMIT);
        self.messages.iter().skip(message_start)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct AgentMessageDTO {
    pub(super) role: String,
    pub(super) text: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct AgentConversationContextDTO {
    pub(super) title: String,
    pub(super) risk_reason: String,
    pub(super) action_summary: String,
    pub(super) evidence_summaries: Vec<String>,
    pub(super) workspace_summary: String,
    pub(super) workspace_signals: Vec<String>,
    pub(super) pending_action_summaries: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_agent_stream_request_reads_workspace_context() {
        let body = r#"{
                "messages": [{"role": "user", "text": "解释风险"}],
                "context": {
                    "title": "KR 风险",
                    "risk_reason": "连续延期",
                    "action_summary": "更新进展",
                    "evidence_summaries": ["连续两周延期"],
                    "workspace_summary": "工作区摘要：共 2 个风险。",
                    "workspace_signals": ["严重｜KR 风险"],
                    "pending_action_summaries": ["KR 风险｜更新进展｜gate：待处理"]
                }
            }"#;
        let request = decode_agent_stream_request(body.as_bytes()).expect("request");

        assert_eq!(request.context.workspace_summary, "工作区摘要：共 2 个风险。");
        assert_eq!(request.context.workspace_signals, vec!["严重｜KR 风险"]);
        assert_eq!(
            request.context.pending_action_summaries,
            vec!["KR 风险｜更新进展｜gate：待处理"]
        );
    }
}

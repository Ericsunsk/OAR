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
}

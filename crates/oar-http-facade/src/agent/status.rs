use serde::Serialize;

use super::context_text::safe_context_summaries;
use super::request::AgentConversationContextDTO;

const AGENT_CONTEXT_STATUS_SUMMARY_LIMIT: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct AgentContextStatus {
    pub(crate) activated_skill_summaries: Vec<String>,
    pub(crate) live_read_summaries: Vec<String>,
}

impl AgentContextStatus {
    pub(in crate::agent) fn from_context(context: &AgentConversationContextDTO) -> Self {
        Self {
            activated_skill_summaries: safe_context_summaries(
                &context.activated_skill_summaries,
                AGENT_CONTEXT_STATUS_SUMMARY_LIMIT,
            ),
            live_read_summaries: safe_context_summaries(
                &context.live_feishu_read_summaries,
                AGENT_CONTEXT_STATUS_SUMMARY_LIMIT,
            ),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.activated_skill_summaries.is_empty() && self.live_read_summaries.is_empty()
    }
}

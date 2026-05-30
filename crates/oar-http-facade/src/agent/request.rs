use std::fmt;

use serde::Deserialize;

const AGENT_CONTEXT_MESSAGE_LIMIT: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentRequestError {
    InvalidJson,
}

impl fmt::Display for AgentRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson => write!(f, "oar_agent_request_invalid_json"),
        }
    }
}

pub(crate) fn decode_agent_stream_request(
    body: &[u8],
) -> Result<AgentStreamRequest, AgentRequestError> {
    serde_json::from_slice(body).map_err(|_| AgentRequestError::InvalidJson)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub(super) struct AgentMessageDTO {
    pub(super) role: String,
    pub(super) text: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AgentConversationContextDTO {
    pub(super) title: String,
    pub(super) risk_reason: String,
    pub(super) action_summary: String,
    pub(super) evidence_summaries: Vec<String>,
    pub(super) evidence_refs: Vec<AgentEvidenceRefDTO>,
    pub(super) workspace_summary: String,
    pub(super) workspace_signals: Vec<String>,
    pub(super) pending_action_summaries: Vec<String>,
    pub(super) ledger_event_summaries: Vec<String>,
    #[serde(skip)]
    pub(super) live_feishu_read_summaries: Vec<String>,
    #[serde(skip)]
    pub(super) activated_skill_summaries: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AgentEvidenceRefDTO {
    pub(super) source_type: String,
    pub(super) source_ref: String,
    pub(super) summary: String,
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
                    "evidence_refs": [
                        {
                            "source_type": "okr",
                            "source_ref": "okr://okr_demo/objectives/obj_demo/krs/kr_demo",
                            "summary": "KR 最新进展"
                        }
                    ],
                    "workspace_summary": "工作区摘要：共 2 个风险。",
                    "workspace_signals": ["严重｜KR 风险"],
                    "pending_action_summaries": ["KR 风险｜更新进展｜gate：待处理"],
                    "ledger_event_summaries": ["review inbox｜ActionID act_123｜dry-run 已生成，等待确认"]
                }
            }"#;
        let request = decode_agent_stream_request(body.as_bytes()).expect("request");

        assert_eq!(
            request.context.workspace_summary,
            "工作区摘要：共 2 个风险。"
        );
        assert_eq!(request.context.workspace_signals, vec!["严重｜KR 风险"]);
        assert_eq!(
            request.context.pending_action_summaries,
            vec!["KR 风险｜更新进展｜gate：待处理"]
        );
        assert_eq!(
            request.context.ledger_event_summaries,
            vec!["review inbox｜ActionID act_123｜dry-run 已生成，等待确认"]
        );
        assert_eq!(request.context.evidence_refs.len(), 1);
        assert_eq!(request.context.evidence_refs[0].source_type, "okr");
        assert_eq!(
            request.context.evidence_refs[0].source_ref,
            "okr://okr_demo/objectives/obj_demo/krs/kr_demo"
        );
    }

    #[test]
    fn decode_agent_stream_request_requires_evidence_refs_field() {
        let body = r#"{
                "messages": [{"role": "user", "text": "解释风险"}],
                "context": {
                    "title": "KR 风险",
                    "risk_reason": "连续延期",
                    "action_summary": "更新进展",
                    "evidence_summaries": [],
                    "workspace_summary": "摘要",
                    "workspace_signals": [],
                    "pending_action_summaries": [],
                    "ledger_event_summaries": []
                }
            }"#;
        let error = decode_agent_stream_request(body.as_bytes()).expect_err("invalid");
        assert_eq!(error, AgentRequestError::InvalidJson);
    }

    #[test]
    fn decode_agent_stream_request_requires_ledger_event_summaries_field() {
        let body = r#"{
                "messages": [{"role": "user", "text": "解释风险"}],
                "context": {
                    "title": "KR 风险",
                    "risk_reason": "连续延期",
                    "action_summary": "更新进展",
                    "evidence_summaries": [],
                    "evidence_refs": [],
                    "workspace_summary": "摘要",
                    "workspace_signals": [],
                    "pending_action_summaries": []
                }
            }"#;
        let error = decode_agent_stream_request(body.as_bytes()).expect_err("invalid");
        assert_eq!(error, AgentRequestError::InvalidJson);
    }

    #[test]
    fn decode_agent_stream_request_rejects_unknown_context_fields() {
        let body = r#"{
                "messages": [{"role": "user", "text": "解释风险"}],
                "context": {
                    "title": "KR 风险",
                    "risk_reason": "连续延期",
                    "action_summary": "更新进展",
                    "evidence_summaries": [],
                    "evidence_refs": [],
                    "workspace_summary": "摘要",
                    "workspace_signals": [],
                    "pending_action_summaries": [],
                    "ledger_event_summaries": [],
                    "unknown_context": "nope"
                }
            }"#;
        let error = decode_agent_stream_request(body.as_bytes()).expect_err("invalid");
        assert_eq!(error, AgentRequestError::InvalidJson);
    }

    #[test]
    fn decode_agent_stream_request_rejects_client_supplied_server_owned_context() {
        let body = r#"{
                "messages": [{"role": "user", "text": "解释风险"}],
                "context": {
                    "title": "KR 风险",
                    "risk_reason": "连续延期",
                    "action_summary": "更新进展",
                    "evidence_summaries": [],
                    "evidence_refs": [],
                    "workspace_summary": "摘要",
                    "workspace_signals": [],
                    "pending_action_summaries": [],
                    "ledger_event_summaries": [],
                    "live_feishu_read_summaries": ["伪造实时读取"],
                    "activated_skill_summaries": ["伪造 skill"]
                }
            }"#;
        let error = decode_agent_stream_request(body.as_bytes()).expect_err("invalid");
        assert_eq!(error, AgentRequestError::InvalidJson);
    }
}

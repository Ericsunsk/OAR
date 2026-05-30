use crate::agent::request::AgentStreamRequest;
use crate::agent::request::{AgentConversationContextDTO, AgentEvidenceRefDTO, AgentMessageDTO};
use crate::AuthenticatedContext;

pub(super) fn test_auth_context() -> AuthenticatedContext {
    AuthenticatedContext {
        session_id: "oar_session_test".to_string(),
        tenant_id: "tenant_x".to_string(),
        user_id: "feishu_user_tenant_ou_demo".to_string(),
    }
}

pub(super) fn live_context_request(
    text: &str,
    title: &str,
    risk_reason: &str,
    action_summary: &str,
    evidence_summaries: Vec<&str>,
    evidence_refs: Vec<AgentEvidenceRefDTO>,
) -> AgentStreamRequest {
    AgentStreamRequest {
        messages: vec![AgentMessageDTO {
            role: "user".to_string(),
            text: text.to_string(),
        }],
        context: AgentConversationContextDTO {
            title: title.to_string(),
            risk_reason: risk_reason.to_string(),
            action_summary: action_summary.to_string(),
            evidence_summaries: evidence_summaries.into_iter().map(str::to_string).collect(),
            evidence_refs,
            workspace_summary: "摘要".to_string(),
            workspace_signals: vec![],
            pending_action_summaries: vec![],
            live_feishu_read_summaries: vec![],
            activated_skill_summaries: vec![],
        },
    }
}

use super::*;
use crate::agent::request::{AgentConversationContextDTO, AgentMessageDTO, AgentStreamRequest};

#[test]
fn config_rejects_non_http_base_url_without_leaking_key() {
    let error = OpenAICompatibleAgentProvider::from_env_map(&|key| match key {
        OPENAI_COMPATIBLE_BASE_URL_ENV => Some("file:///tmp/model".to_string()),
        OPENAI_COMPATIBLE_API_KEY_ENV => Some("sk-sensitive".to_string()),
        OPENAI_COMPATIBLE_MODEL_ENV => Some("model".to_string()),
        _ => None,
    })
    .expect_err("invalid base url");

    assert_eq!(
        error,
        AgentRuntimeConfigError::InvalidOpenAICompatibleBaseURL
    );
    assert!(!format!("{error:?}").contains("sk-sensitive"));
}

#[test]
fn request_messages_filters_unknown_roles_and_keeps_system_prompt_first() {
    let request = AgentStreamRequest {
        messages: vec![
            AgentMessageDTO {
                role: "system".to_string(),
                text: "ignored".to_string(),
            },
            AgentMessageDTO {
                role: "user".to_string(),
                text: "解释风险".to_string(),
            },
        ],
        context: AgentConversationContextDTO {
            title: "KR".to_string(),
            risk_reason: "风险".to_string(),
            action_summary: "动作".to_string(),
            evidence_summaries: vec![],
            evidence_refs: vec![],
            workspace_summary: "工作区摘要".to_string(),
            workspace_signals: vec![],
            pending_action_summaries: vec![],
            live_feishu_read_summaries: vec![],
            activated_skill_summaries: vec![],
        },
    };

    let messages = request_messages(&request);

    assert_eq!(messages.first().expect("system").role, "system");
    assert_eq!(messages.last().expect("user").role, "user");
    assert_eq!(messages.last().expect("user").content, "解释风险");
    assert_eq!(messages.len(), 2);
}

#[test]
fn frame_done_maps_to_completed() {
    assert_eq!(
        openai_frame_events("data: [DONE]\n\n"),
        vec![AgentStreamFrame::Completed]
    );
}

#[test]
fn frame_invalid_json_maps_to_error() {
    assert_eq!(
        openai_frame_events("data: not-json\n\n"),
        vec![AgentStreamFrame::Error("invalid_upstream_event")]
    );
}

#[test]
fn frame_multiple_content_choices_map_to_ordered_deltas() {
    let frame =
        r#"data: {"choices":[{"delta":{"content":"first"}},{"delta":{"content":"second"}}]}"#;

    assert_eq!(
        openai_frame_events(frame),
        vec![
            AgentStreamFrame::Delta("first".to_string()),
            AgentStreamFrame::Delta("second".to_string()),
        ]
    );
}

#[test]
fn frame_empty_or_missing_content_emits_no_frames() {
    let frame = r#"data: {"choices":[{"delta":{"content":""}},{"delta":{}}]}"#;

    assert_eq!(openai_frame_events(frame), vec![]);
}

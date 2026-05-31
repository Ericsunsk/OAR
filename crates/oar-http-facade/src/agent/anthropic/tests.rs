use std::convert::Infallible;

use bytes::Bytes;
use hyper::body::Frame;
use tokio::sync::mpsc;

use super::*;
use crate::agent::request::{AgentConversationContextDTO, AgentMessageDTO, AgentStreamRequest};

#[tokio::test]
async fn anthropic_frame_maps_text_delta_and_message_stop() {
    let (sender, mut receiver) = mpsc::channel::<Result<Frame<Bytes>, Infallible>>(4);

    send_anthropic_frame(
        &sender,
        r#"data: {"type":"content_block_delta","delta":{"type":"text_delta","text":"hello"}}"#,
    )
    .await
    .expect("delta frame");
    send_anthropic_frame(&sender, r#"data: {"type":"message_stop"}"#)
        .await
        .expect("stop frame");

    let delta = receiver.recv().await.expect("delta").expect("frame");
    let stop = receiver.recv().await.expect("stop").expect("frame");
    let delta = String::from_utf8(delta.into_data().expect("data").to_vec()).expect("utf8");
    let stop = String::from_utf8(stop.into_data().expect("data").to_vec()).expect("utf8");

    assert!(delta.contains(r#""event":"delta""#));
    assert!(delta.contains(r#""delta":"hello""#));
    assert!(stop.contains(r#""event":"completed""#));
}

#[test]
fn anthropic_frame_events_maps_invalid_json_to_error() {
    assert_eq!(
        anthropic_frame_events("data: {not-json"),
        AgentStreamFrame::invalid_upstream_event()
    );
}

#[test]
fn anthropic_frame_events_maps_upstream_error_to_error() {
    assert_eq!(
        anthropic_frame_events(r#"data: {"type":"error"}"#),
        vec![AgentStreamFrame::Error("upstream_error")]
    );
}

#[test]
fn anthropic_frame_events_ignores_non_text_delta() {
    assert_eq!(
        anthropic_frame_events(
            r#"data: {"type":"content_block_delta","delta":{"type":"input_json_delta","text":"ignored"}}"#,
        ),
        Vec::<AgentStreamFrame>::new()
    );
}

#[test]
fn anthropic_frame_events_ignores_missing_delta_or_empty_text() {
    assert_eq!(
        anthropic_frame_events(r#"data: {"type":"content_block_delta"}"#),
        Vec::<AgentStreamFrame>::new()
    );
    assert_eq!(
        anthropic_frame_events(
            r#"data: {"type":"content_block_delta","delta":{"type":"text_delta","text":""}}"#,
        ),
        Vec::<AgentStreamFrame>::new()
    );
}

#[test]
fn anthropic_messages_drop_leading_assistant_and_merge_adjacent_roles() {
    let request = AgentStreamRequest {
        messages: vec![
            AgentMessageDTO {
                role: "assistant".to_string(),
                text: "initial helper".to_string(),
            },
            AgentMessageDTO {
                role: "user".to_string(),
                text: "解释风险".to_string(),
            },
            AgentMessageDTO {
                role: "user".to_string(),
                text: "补充证据".to_string(),
            },
            AgentMessageDTO {
                role: "assistant".to_string(),
                text: "收到".to_string(),
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
            ledger_event_summaries: vec![],
            live_feishu_read_summaries: vec![],
            activated_skill_summaries: vec![],
        },
    };

    let messages = anthropic_messages(&request);

    assert_eq!(
        messages,
        vec![
            AnthropicMessageDTO {
                role: "user".to_string(),
                content: "解释风险\n\n补充证据".to_string(),
            },
            AnthropicMessageDTO {
                role: "assistant".to_string(),
                content: "收到".to_string(),
            },
        ]
    );
}

use std::fmt;

use reqwest::Url;
use serde::{Deserialize, Serialize};

use super::prompt::AgentSystemPromptBuilder;
use super::request::AgentStreamRequest;
use super::stream::{
    agent_frame_channel, sse_data_payload, stream_upstream_sse_response, AgentFrameStream,
    AgentStreamFrame,
};
#[cfg(test)]
use super::stream::{send_agent_stream_frames, AgentFrameSendError, AgentFrameSender};
use super::{
    agent_http_client, ensure_successful_upstream_response, is_allowed_agent_base_url,
    non_empty_env, AgentProviderConfig, AgentProviderConfigSummary, AgentRuntimeConfigError,
    AgentStreamError,
};

const ANTHROPIC_BASE_URL_ENV: &str = "OAR_AGENT_ANTHROPIC_BASE_URL";
const ANTHROPIC_API_KEY_ENV: &str = "OAR_AGENT_ANTHROPIC_API_KEY";
const ANTHROPIC_MODEL_ENV: &str = "OAR_AGENT_ANTHROPIC_MODEL";
const ANTHROPIC_VERSION_ENV: &str = "OAR_AGENT_ANTHROPIC_VERSION";
const DEFAULT_ANTHROPIC_BASE_URL: &str = "https://api.anthropic.com/v1";
const DEFAULT_ANTHROPIC_VERSION: &str = "2023-06-01";
const DEFAULT_ANTHROPIC_MAX_TOKENS: u32 = 1_024;

#[derive(Clone)]
pub(super) struct AnthropicAgentProvider {
    client: reqwest::Client,
    base_url: Url,
    api_key: String,
    model: String,
    version: String,
    max_tokens: u32,
}

impl fmt::Debug for AnthropicAgentProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AnthropicAgentProvider")
            .field("base_url", &self.base_url.as_str())
            .field("api_key", &"[REDACTED]")
            .field("model", &self.model)
            .field("version", &self.version)
            .field("max_tokens", &self.max_tokens)
            .finish()
    }
}

impl AnthropicAgentProvider {
    pub(super) fn from_provider_config(
        config: AgentProviderConfig,
    ) -> Result<Self, AgentRuntimeConfigError> {
        Ok(Self {
            client: agent_http_client()?,
            base_url: config.base_url,
            api_key: config.api_key,
            model: config.model,
            version: config
                .anthropic_version
                .unwrap_or_else(|| DEFAULT_ANTHROPIC_VERSION.to_string()),
            max_tokens: DEFAULT_ANTHROPIC_MAX_TOKENS,
        })
    }

    pub(super) fn config_summary(&self) -> AgentProviderConfigSummary {
        AgentProviderConfigSummary {
            protocol: "anthropic",
            base_url: self.base_url.as_str().to_string(),
            model: self.model.clone(),
        }
    }

    pub(super) fn has_any_env_config(env: &impl Fn(&str) -> Option<String>) -> bool {
        non_empty_env(env, ANTHROPIC_BASE_URL_ENV).is_some()
            || non_empty_env(env, ANTHROPIC_API_KEY_ENV).is_some()
            || non_empty_env(env, ANTHROPIC_MODEL_ENV).is_some()
            || non_empty_env(env, ANTHROPIC_VERSION_ENV).is_some()
    }

    pub(super) fn from_env_map(
        env: &impl Fn(&str) -> Option<String>,
    ) -> Result<Option<Self>, AgentRuntimeConfigError> {
        if !Self::has_any_env_config(env) {
            return Ok(None);
        }

        let base_url = non_empty_env(env, ANTHROPIC_BASE_URL_ENV)
            .unwrap_or_else(|| DEFAULT_ANTHROPIC_BASE_URL.to_string());
        let api_key = non_empty_env(env, ANTHROPIC_API_KEY_ENV);
        let model = non_empty_env(env, ANTHROPIC_MODEL_ENV);
        let (Some(api_key), Some(model)) = (api_key, model) else {
            return Err(AgentRuntimeConfigError::PartialAnthropicConfig);
        };
        let version = non_empty_env(env, ANTHROPIC_VERSION_ENV)
            .unwrap_or_else(|| DEFAULT_ANTHROPIC_VERSION.to_string());
        let base_url = Url::parse(&base_url)
            .ok()
            .filter(is_allowed_agent_base_url)
            .ok_or(AgentRuntimeConfigError::InvalidAnthropicBaseURL)?;
        let client = agent_http_client()?;

        Ok(Some(Self {
            client,
            base_url,
            api_key,
            model,
            version,
            max_tokens: DEFAULT_ANTHROPIC_MAX_TOKENS,
        }))
    }

    pub(super) async fn open_stream(
        &self,
        request: AgentStreamRequest,
    ) -> Result<AgentFrameStream, AgentStreamError> {
        let upstream_request = AnthropicMessagesRequestDTO {
            model: &self.model,
            max_tokens: self.max_tokens,
            system: AgentSystemPromptBuilder::default().make_prompt(&request.context),
            messages: anthropic_messages(&request),
            temperature: 0.2,
            stream: true,
        };
        let response = self
            .client
            .post(anthropic_messages_url(&self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", &self.version)
            .header("Accept", "text/event-stream")
            .json(&upstream_request)
            .send()
            .await
            .map_err(|_| AgentStreamError::UpstreamUnavailable)?;

        ensure_successful_upstream_response(&response)?;

        let (sender, receiver) = agent_frame_channel();
        tokio::spawn(stream_upstream_sse_response(
            response.bytes_stream(),
            sender,
            anthropic_frame_events,
        ));
        Ok(receiver)
    }
}

#[derive(Debug, Serialize)]
struct AnthropicMessagesRequestDTO<'a> {
    model: &'a str,
    max_tokens: u32,
    system: String,
    messages: Vec<AnthropicMessageDTO>,
    temperature: f64,
    stream: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct AnthropicMessageDTO {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicStreamEventDTO {
    #[serde(rename = "type")]
    event_type: String,
    delta: Option<AnthropicStreamDeltaDTO>,
}

#[derive(Debug, Deserialize)]
struct AnthropicStreamDeltaDTO {
    #[serde(rename = "type")]
    delta_type: Option<String>,
    text: Option<String>,
}

fn anthropic_messages(request: &AgentStreamRequest) -> Vec<AnthropicMessageDTO> {
    let mut messages: Vec<AnthropicMessageDTO> = Vec::new();
    for message in request.recent_messages() {
        let role = match message.role.as_str() {
            "assistant" => "assistant",
            "user" => "user",
            _ => continue,
        };
        let text = message.text.trim();
        if text.is_empty() {
            continue;
        }
        if messages.is_empty() && role == "assistant" {
            continue;
        }
        if let Some(previous) = messages.last_mut() {
            if previous.role == role {
                previous.content.push_str("\n\n");
                previous.content.push_str(text);
                continue;
            }
        }
        messages.push(AnthropicMessageDTO {
            role: role.to_string(),
            content: text.to_string(),
        });
    }
    messages
}

fn anthropic_messages_url(base_url: &Url) -> Url {
    let mut endpoint = base_url.clone();
    let path = format!("{}/messages", endpoint.path().trim_end_matches('/'));
    endpoint.set_path(&path);
    endpoint
}

#[cfg(test)]
async fn send_anthropic_frame(
    sender: &AgentFrameSender,
    frame: &str,
) -> Result<(), AgentFrameSendError> {
    send_agent_stream_frames(sender, anthropic_frame_events(frame)).await
}

fn anthropic_frame_events(frame: &str) -> Vec<AgentStreamFrame> {
    let Some(payload) = sse_data_payload(frame) else {
        return vec![];
    };
    let Ok(event) = serde_json::from_str::<AnthropicStreamEventDTO>(&payload) else {
        return vec![AgentStreamFrame::Error("invalid_upstream_event")];
    };

    match event.event_type.as_str() {
        "content_block_delta" => {
            let Some(delta) = event.delta else {
                return vec![];
            };
            if delta.delta_type.as_deref() == Some("text_delta") {
                if let Some(text) = delta.text.filter(|value| !value.is_empty()) {
                    return vec![AgentStreamFrame::Delta(text)];
                }
            }
            vec![]
        }
        "message_stop" => vec![AgentStreamFrame::Completed],
        "error" => vec![AgentStreamFrame::Error("upstream_error")],
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
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
                workspace_summary: "工作区摘要".to_string(),
                workspace_signals: vec![],
                pending_action_summaries: vec![],
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
}

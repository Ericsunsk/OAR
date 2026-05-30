use std::fmt;

use reqwest::Url;
use serde::{Deserialize, Serialize};

use super::prompt::AgentSystemPromptBuilder;
use super::request::AgentStreamRequest;
#[cfg(test)]
use super::stream::{send_agent_stream_frames, AgentFrameSendError, AgentFrameSender};
use super::stream::{
    spawn_upstream_sse_response, sse_data_payload, AgentFrameStream, AgentStreamFrame,
};
use super::{
    agent_endpoint_url, agent_http_client, ensure_successful_upstream_response,
    is_allowed_agent_base_url, AgentProviderConfig, AgentProviderConfigSummary,
    AgentRuntimeConfigError, AgentStreamError, DEFAULT_ANTHROPIC_VERSION,
};
use crate::util::non_empty_env;

const ANTHROPIC_BASE_URL_ENV: &str = "OAR_AGENT_ANTHROPIC_BASE_URL";
const ANTHROPIC_API_KEY_ENV: &str = "OAR_AGENT_ANTHROPIC_API_KEY";
const ANTHROPIC_MODEL_ENV: &str = "OAR_AGENT_ANTHROPIC_MODEL";
const ANTHROPIC_VERSION_ENV: &str = "OAR_AGENT_ANTHROPIC_VERSION";
const DEFAULT_ANTHROPIC_BASE_URL: &str = "https://api.anthropic.com/v1";
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
            system: AgentSystemPromptBuilder::make_prompt(&request.context),
            messages: anthropic_messages(&request),
            temperature: 0.2,
            stream: true,
        };
        let response = self
            .client
            .post(agent_endpoint_url(&self.base_url, "messages"))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", &self.version)
            .header("Accept", "text/event-stream")
            .json(&upstream_request)
            .send()
            .await
            .map_err(|_| AgentStreamError::UpstreamUnavailable)?;

        ensure_successful_upstream_response(&response)?;

        Ok(spawn_upstream_sse_response(
            response,
            anthropic_frame_events,
        ))
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
mod tests;

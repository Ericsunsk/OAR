use std::fmt;

use reqwest::Url;
use serde::{Deserialize, Serialize};

use super::prompt::AgentSystemPromptBuilder;
use super::request::AgentStreamRequest;
use super::stream::{
    spawn_upstream_sse_response, sse_data_payload, AgentFrameStream, AgentStreamFrame,
};
use super::{
    agent_endpoint_url, agent_http_client, ensure_successful_upstream_response,
    is_allowed_agent_base_url, AgentProviderConfig, AgentProviderConfigSummary,
    AgentRuntimeConfigError, AgentStreamError,
};
use crate::util::non_empty_env;

const OPENAI_COMPATIBLE_BASE_URL_ENV: &str = "OAR_AGENT_OPENAI_BASE_URL";
const OPENAI_COMPATIBLE_API_KEY_ENV: &str = "OAR_AGENT_OPENAI_API_KEY";
const OPENAI_COMPATIBLE_MODEL_ENV: &str = "OAR_AGENT_OPENAI_MODEL";

#[derive(Clone)]
pub(super) struct OpenAICompatibleAgentProvider {
    client: reqwest::Client,
    base_url: Url,
    api_key: String,
    model: String,
}

impl fmt::Debug for OpenAICompatibleAgentProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenAICompatibleAgentProvider")
            .field("base_url", &self.base_url.as_str())
            .field("api_key", &"[REDACTED]")
            .field("model", &self.model)
            .finish()
    }
}

impl OpenAICompatibleAgentProvider {
    pub(super) fn from_provider_config(
        config: AgentProviderConfig,
    ) -> Result<Self, AgentRuntimeConfigError> {
        Ok(Self {
            client: agent_http_client()?,
            base_url: config.base_url,
            api_key: config.api_key,
            model: config.model,
        })
    }

    pub(super) fn config_summary(&self) -> AgentProviderConfigSummary {
        AgentProviderConfigSummary {
            protocol: "openai-compatible",
            base_url: self.base_url.as_str().to_string(),
            model: self.model.clone(),
        }
    }

    pub(super) fn has_any_env_config(env: &impl Fn(&str) -> Option<String>) -> bool {
        non_empty_env(env, OPENAI_COMPATIBLE_BASE_URL_ENV).is_some()
            || non_empty_env(env, OPENAI_COMPATIBLE_API_KEY_ENV).is_some()
            || non_empty_env(env, OPENAI_COMPATIBLE_MODEL_ENV).is_some()
    }

    pub(super) fn from_env_map(
        env: &impl Fn(&str) -> Option<String>,
    ) -> Result<Option<Self>, AgentRuntimeConfigError> {
        let base_url = non_empty_env(env, OPENAI_COMPATIBLE_BASE_URL_ENV);
        let api_key = non_empty_env(env, OPENAI_COMPATIBLE_API_KEY_ENV);
        let model = non_empty_env(env, OPENAI_COMPATIBLE_MODEL_ENV);
        let has_any_config = Self::has_any_env_config(env);
        if !has_any_config {
            return Ok(None);
        }

        let (Some(base_url), Some(api_key), Some(model)) = (base_url, api_key, model) else {
            return Err(AgentRuntimeConfigError::PartialOpenAICompatibleConfig);
        };

        let base_url = Url::parse(&base_url)
            .ok()
            .filter(is_allowed_agent_base_url)
            .ok_or(AgentRuntimeConfigError::InvalidOpenAICompatibleBaseURL)?;
        let client = agent_http_client()?;

        Ok(Some(Self {
            client,
            base_url,
            api_key,
            model,
        }))
    }

    pub(super) async fn open_stream(
        &self,
        request: AgentStreamRequest,
    ) -> Result<AgentFrameStream, AgentStreamError> {
        let upstream_request = OpenAIChatCompletionRequestDTO {
            model: &self.model,
            messages: request_messages(&request),
            temperature: 0.2,
            stream: true,
        };
        let response = self
            .client
            .post(agent_endpoint_url(&self.base_url, "chat/completions"))
            .bearer_auth(&self.api_key)
            .header("Accept", "text/event-stream")
            .json(&upstream_request)
            .send()
            .await
            .map_err(|_| AgentStreamError::UpstreamUnavailable)?;

        ensure_successful_upstream_response(&response)?;

        Ok(spawn_upstream_sse_response(response, openai_frame_events))
    }
}

#[derive(Debug, Serialize)]
struct OpenAIChatCompletionRequestDTO<'a> {
    model: &'a str,
    messages: Vec<OpenAIChatMessageDTO>,
    temperature: f64,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct OpenAIChatMessageDTO {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIChatCompletionStreamChunkDTO {
    choices: Vec<OpenAIChatCompletionChoiceDTO>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChatCompletionChoiceDTO {
    delta: OpenAIChatCompletionDeltaDTO,
}

#[derive(Debug, Deserialize)]
struct OpenAIChatCompletionDeltaDTO {
    content: Option<String>,
}

fn request_messages(request: &AgentStreamRequest) -> Vec<OpenAIChatMessageDTO> {
    let mut messages = vec![OpenAIChatMessageDTO {
        role: "system".to_string(),
        content: AgentSystemPromptBuilder::make_prompt(&request.context),
    }];
    messages.extend(request.recent_messages().filter_map(|message| {
        let role = match message.role.as_str() {
            "assistant" => "assistant",
            "user" => "user",
            _ => return None,
        };
        let text = message.text.trim();
        if text.is_empty() {
            return None;
        }
        Some(OpenAIChatMessageDTO {
            role: role.to_string(),
            content: text.to_string(),
        })
    }));
    messages
}

fn openai_frame_events(frame: &str) -> Vec<AgentStreamFrame> {
    let Some(payload) = sse_data_payload(frame) else {
        return vec![];
    };
    if payload == "[DONE]" {
        return vec![AgentStreamFrame::Completed];
    }

    let Ok(chunk) = serde_json::from_str::<OpenAIChatCompletionStreamChunkDTO>(&payload) else {
        return vec![AgentStreamFrame::Error("invalid_upstream_event")];
    };
    chunk
        .choices
        .into_iter()
        .filter_map(|choice| choice.delta.content.filter(|value| !value.is_empty()))
        .map(AgentStreamFrame::Delta)
        .collect()
}

#[cfg(test)]
mod tests;

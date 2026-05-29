mod anthropic;
mod live_context;
mod openai;
mod prompt;
mod request;
mod settings;
mod skills;
mod stream;
mod tools;

use std::fmt;
use std::time::Duration;

use reqwest::Url;

use anthropic::AnthropicAgentProvider;
pub(crate) use live_context::inject_live_feishu_context;
use openai::OpenAICompatibleAgentProvider;
pub(crate) use request::{decode_agent_stream_request, AgentStreamRequest};
pub(crate) use settings::{
    decode_agent_model_catalog_request, decode_agent_settings_update_request,
    AgentModelCatalogRequest, AgentModelSettingsError, AgentModelSettingsRuntime,
    AgentSettingsUpdateRequest,
};
pub(crate) use stream::AgentFrameStream;

use crate::util::non_empty_env;

const AGENT_PROVIDER_ENV: &str = "OAR_AGENT_PROVIDER";
const AGENT_HTTP_TIMEOUT: Duration = Duration::from_secs(90);
const DEFAULT_ANTHROPIC_VERSION: &str = "2023-06-01";

#[derive(Clone)]
pub(crate) struct AgentRuntime {
    provider: AgentProvider,
}

impl fmt::Debug for AgentRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgentRuntime")
            .field("provider", &self.provider)
            .finish()
    }
}

impl AgentRuntime {
    pub(crate) fn from_env_map(
        env: &impl Fn(&str) -> Option<String>,
    ) -> Result<Option<Self>, AgentRuntimeConfigError> {
        AgentProvider::from_env_map(env).map(|provider| provider.map(|provider| Self { provider }))
    }

    pub(crate) async fn open_stream(
        &self,
        request: AgentStreamRequest,
    ) -> Result<AgentFrameStream, AgentStreamError> {
        self.provider.open_stream(request).await
    }

    pub(crate) fn from_provider_config(
        config: AgentProviderConfig,
    ) -> Result<Self, AgentRuntimeConfigError> {
        AgentProvider::from_provider_config(config).map(|provider| Self { provider })
    }

    pub(crate) fn config_summary(&self) -> AgentProviderConfigSummary {
        self.provider.config_summary()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentRuntimeConfigError {
    PartialOpenAICompatibleConfig,
    InvalidOpenAICompatibleBaseURL,
    PartialAnthropicConfig,
    InvalidAnthropicBaseURL,
    InvalidAgentProvider,
    AmbiguousAgentProviderConfig,
    HttpClientBuildFailed,
}

impl fmt::Display for AgentRuntimeConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PartialOpenAICompatibleConfig => {
                write!(f, "oar_agent_openai_compatible_config_partial")
            }
            Self::InvalidOpenAICompatibleBaseURL => {
                write!(f, "oar_agent_openai_compatible_base_url_invalid")
            }
            Self::PartialAnthropicConfig => write!(f, "oar_agent_anthropic_config_partial"),
            Self::InvalidAnthropicBaseURL => write!(f, "oar_agent_anthropic_base_url_invalid"),
            Self::InvalidAgentProvider => write!(f, "oar_agent_provider_invalid"),
            Self::AmbiguousAgentProviderConfig => {
                write!(f, "oar_agent_provider_config_ambiguous")
            }
            Self::HttpClientBuildFailed => write!(f, "oar_agent_http_client_build_failed"),
        }
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentStreamError {
    UpstreamUnauthorized,
    UpstreamUnavailable,
}

impl fmt::Display for AgentStreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UpstreamUnauthorized => write!(f, "oar_agent_upstream_unauthorized"),
            Self::UpstreamUnavailable => write!(f, "oar_agent_upstream_unavailable"),
        }
    }
}

#[derive(Clone)]
enum AgentProvider {
    OpenAICompatible(OpenAICompatibleAgentProvider),
    Anthropic(AnthropicAgentProvider),
}

impl fmt::Debug for AgentProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OpenAICompatible(provider) => f
                .debug_tuple("AgentProvider::OpenAICompatible")
                .field(provider)
                .finish(),
            Self::Anthropic(provider) => f
                .debug_tuple("AgentProvider::Anthropic")
                .field(provider)
                .finish(),
        }
    }
}

impl AgentProvider {
    fn from_env_map(
        env: &impl Fn(&str) -> Option<String>,
    ) -> Result<Option<Self>, AgentRuntimeConfigError> {
        let provider = non_empty_env(env, AGENT_PROVIDER_ENV).map(|value| value.to_lowercase());
        match provider.as_deref() {
            Some("openai") | Some("openai-compatible") | Some("openai_compatible") => {
                match OpenAICompatibleAgentProvider::from_env_map(env)? {
                    Some(provider) => Ok(Some(Self::OpenAICompatible(provider))),
                    None => Err(AgentRuntimeConfigError::PartialOpenAICompatibleConfig),
                }
            }
            Some("anthropic") | Some("claude") => {
                match AnthropicAgentProvider::from_env_map(env)? {
                    Some(provider) => Ok(Some(Self::Anthropic(provider))),
                    None => Err(AgentRuntimeConfigError::PartialAnthropicConfig),
                }
            }
            Some(_) => Err(AgentRuntimeConfigError::InvalidAgentProvider),
            None => {
                let has_openai_config = OpenAICompatibleAgentProvider::has_any_env_config(env);
                let has_anthropic_config = AnthropicAgentProvider::has_any_env_config(env);
                match (has_openai_config, has_anthropic_config) {
                    (false, false) => Ok(None),
                    (true, false) => OpenAICompatibleAgentProvider::from_env_map(env)
                        .map(|provider| provider.map(Self::OpenAICompatible)),
                    (false, true) => AnthropicAgentProvider::from_env_map(env)
                        .map(|provider| provider.map(Self::Anthropic)),
                    (true, true) => Err(AgentRuntimeConfigError::AmbiguousAgentProviderConfig),
                }
            }
        }
    }

    async fn open_stream(
        &self,
        request: AgentStreamRequest,
    ) -> Result<AgentFrameStream, AgentStreamError> {
        match self {
            Self::OpenAICompatible(provider) => provider.open_stream(request).await,
            Self::Anthropic(provider) => provider.open_stream(request).await,
        }
    }

    fn from_provider_config(config: AgentProviderConfig) -> Result<Self, AgentRuntimeConfigError> {
        match config.protocol {
            AgentProtocol::OpenAICompatible => Ok(Self::OpenAICompatible(
                OpenAICompatibleAgentProvider::from_provider_config(config)?,
            )),
            AgentProtocol::Anthropic => Ok(Self::Anthropic(
                AnthropicAgentProvider::from_provider_config(config)?,
            )),
        }
    }

    fn config_summary(&self) -> AgentProviderConfigSummary {
        match self {
            Self::OpenAICompatible(provider) => provider.config_summary(),
            Self::Anthropic(provider) => provider.config_summary(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AgentProtocol {
    OpenAICompatible,
    Anthropic,
}

impl AgentProtocol {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::OpenAICompatible => "openai-compatible",
            Self::Anthropic => "anthropic",
        }
    }

    pub(super) fn from_str(value: &str) -> Option<Self> {
        match value {
            "openai-compatible" => Some(Self::OpenAICompatible),
            "anthropic" => Some(Self::Anthropic),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub(super) struct AgentProviderConfig {
    protocol: AgentProtocol,
    base_url: Url,
    api_key: String,
    model: String,
    anthropic_version: Option<String>,
}

impl fmt::Debug for AgentProviderConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgentProviderConfig")
            .field("protocol", &self.protocol)
            .field("base_url", &self.base_url.as_str())
            .field("api_key", &"[REDACTED]")
            .field("model", &self.model)
            .field("anthropic_version", &self.anthropic_version)
            .finish()
    }
}

impl AgentProviderConfig {
    pub(super) fn new(
        protocol: AgentProtocol,
        base_url: Url,
        api_key: String,
        model: String,
        anthropic_version: Option<String>,
    ) -> Self {
        Self {
            protocol,
            base_url,
            api_key,
            model,
            anthropic_version,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentProviderConfigSummary {
    pub(crate) protocol: &'static str,
    pub(crate) base_url: String,
    pub(crate) model: String,
}

fn is_allowed_agent_base_url(url: &Url) -> bool {
    match url.scheme() {
        "https" => true,
        "http" => matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1")),
        _ => false,
    }
}

fn agent_http_client() -> Result<reqwest::Client, AgentRuntimeConfigError> {
    reqwest::Client::builder()
        .timeout(AGENT_HTTP_TIMEOUT)
        .build()
        .map_err(|_| AgentRuntimeConfigError::HttpClientBuildFailed)
}

fn ensure_successful_upstream_response(
    response: &reqwest::Response,
) -> Result<(), AgentStreamError> {
    match response.status().as_u16() {
        200..=299 => Ok(()),
        401 | 403 => Err(AgentStreamError::UpstreamUnauthorized),
        _ => Err(AgentStreamError::UpstreamUnavailable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_disables_agent_when_env_absent_and_rejects_partial_config() {
        let disabled = AgentRuntime::from_env_map(&|_| None).expect("disabled");
        assert!(disabled.is_none());

        let partial = AgentRuntime::from_env_map(&|key| {
            (key == "OAR_AGENT_OPENAI_API_KEY").then(|| "sk-sensitive".to_string())
        })
        .expect_err("partial config");

        assert_eq!(
            partial,
            AgentRuntimeConfigError::PartialOpenAICompatibleConfig
        );
        assert!(!format!("{partial:?}").contains("sk-sensitive"));
    }

    #[test]
    fn config_accepts_anthropic_provider_with_defaults_without_leaking_key() {
        let runtime = AgentRuntime::from_env_map(&|key| match key {
            AGENT_PROVIDER_ENV => Some("anthropic".to_string()),
            "OAR_AGENT_ANTHROPIC_API_KEY" => Some("sk-ant-sensitive".to_string()),
            "OAR_AGENT_ANTHROPIC_MODEL" => Some("claude-sonnet-test".to_string()),
            _ => None,
        })
        .expect("anthropic runtime")
        .expect("configured runtime");

        let debug = format!("{runtime:?}");
        assert!(debug.contains("AnthropicAgentProvider"));
        assert!(debug.contains("2023-06-01"));
        assert!(!debug.contains("sk-ant-sensitive"));
    }

    #[test]
    fn config_rejects_explicit_anthropic_provider_without_required_fields() {
        let error = AgentRuntime::from_env_map(&|key| {
            (key == AGENT_PROVIDER_ENV).then(|| "anthropic".to_string())
        })
        .expect_err("anthropic requires key and model");

        assert_eq!(error, AgentRuntimeConfigError::PartialAnthropicConfig);
    }

    #[test]
    fn config_rejects_ambiguous_provider_config_without_leaking_keys() {
        let error = AgentRuntime::from_env_map(&|key| match key {
            "OAR_AGENT_OPENAI_BASE_URL" => Some("https://llm.example.test/v1".to_string()),
            "OAR_AGENT_OPENAI_API_KEY" => Some("sk-openai-sensitive".to_string()),
            "OAR_AGENT_OPENAI_MODEL" => Some("openai-model".to_string()),
            "OAR_AGENT_ANTHROPIC_API_KEY" => Some("sk-ant-sensitive".to_string()),
            "OAR_AGENT_ANTHROPIC_MODEL" => Some("claude-model".to_string()),
            _ => None,
        })
        .expect_err("ambiguous provider config");

        assert_eq!(error, AgentRuntimeConfigError::AmbiguousAgentProviderConfig);
        let debug = format!("{error:?}");
        assert!(!debug.contains("sk-openai-sensitive"));
        assert!(!debug.contains("sk-ant-sensitive"));
    }
}

use std::fmt;
use std::time::Duration;

use reqwest::Url;

use super::{AgentRuntimeConfigError, AgentStreamError};

pub(in crate::agent) const AGENT_PROVIDER_ENV: &str = "OAR_AGENT_PROVIDER";
const AGENT_HTTP_TIMEOUT: Duration = Duration::from_secs(90);
pub(in crate::agent) const DEFAULT_ANTHROPIC_VERSION: &str = "2023-06-01";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::agent) enum AgentProtocol {
    OpenAICompatible,
    Anthropic,
}

impl AgentProtocol {
    pub(in crate::agent) fn as_str(self) -> &'static str {
        match self {
            Self::OpenAICompatible => "openai-compatible",
            Self::Anthropic => "anthropic",
        }
    }

    pub(in crate::agent) fn from_str(value: &str) -> Option<Self> {
        match value {
            "openai-compatible" => Some(Self::OpenAICompatible),
            "anthropic" => Some(Self::Anthropic),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub(crate) struct AgentProviderConfig {
    pub(in crate::agent) protocol: AgentProtocol,
    pub(in crate::agent) base_url: Url,
    pub(in crate::agent) api_key: String,
    pub(in crate::agent) model: String,
    pub(in crate::agent) anthropic_version: Option<String>,
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
    pub(in crate::agent) fn new(
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

pub(in crate::agent) fn is_allowed_agent_base_url(url: &Url) -> bool {
    match url.scheme() {
        "https" => true,
        "http" => matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1")),
        _ => false,
    }
}

pub(in crate::agent) fn agent_http_client() -> Result<reqwest::Client, AgentRuntimeConfigError> {
    reqwest::Client::builder()
        .timeout(AGENT_HTTP_TIMEOUT)
        .build()
        .map_err(|_| AgentRuntimeConfigError::HttpClientBuildFailed)
}

pub(in crate::agent) fn ensure_successful_upstream_response(
    response: &reqwest::Response,
) -> Result<(), AgentStreamError> {
    match response.status().as_u16() {
        200..=299 => Ok(()),
        401 | 403 => Err(AgentStreamError::UpstreamUnauthorized),
        _ => Err(AgentStreamError::UpstreamUnavailable),
    }
}

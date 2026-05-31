mod activation;
mod anthropic;
mod context_text;
mod live_context;
mod openai;
mod prompt;
mod provider_config;
mod request;
mod runtime;
mod settings;
mod skills;
mod status;
mod stream;
mod tools;

pub(crate) use live_context::inject_live_feishu_context;
pub(in crate::agent) use provider_config::{
    agent_endpoint_url, agent_http_client, ensure_successful_upstream_response,
    is_allowed_agent_base_url, AgentProtocol, AGENT_PROVIDER_ENV, DEFAULT_ANTHROPIC_VERSION,
};
pub(crate) use provider_config::{AgentProviderConfig, AgentProviderConfigSummary};
pub(crate) use request::{decode_agent_stream_request, AgentRequestError, AgentStreamRequest};
pub(crate) use runtime::{AgentRuntime, AgentRuntimeConfigError, AgentStreamError};
pub(crate) use settings::{
    decode_agent_model_catalog_request, decode_agent_settings_update_request,
    AgentModelCatalogRequest, AgentModelSettingsError, AgentModelSettingsRuntime,
    AgentSettingsSnapshot, AgentSettingsUpdateRequest,
};
pub(crate) use stream::prepend_agent_context_status_frame;

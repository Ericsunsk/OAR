use serde::{de::DeserializeOwned, Deserialize, Serialize};

use super::AgentModelSettingsError;
use crate::agent::AgentProviderConfigSummary;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AgentSettingsSnapshot {
    pub(crate) source: &'static str,
    pub(crate) detected_protocol: Option<String>,
    pub(crate) base_url: Option<String>,
    pub(crate) selected_model: Option<String>,
    pub(crate) api_key_status: &'static str,
    pub(crate) can_configure: bool,
}

impl AgentSettingsSnapshot {
    pub(crate) fn from_summary(summary: AgentProviderConfigSummary, can_configure: bool) -> Self {
        Self {
            source: "env",
            detected_protocol: Some(summary.protocol.to_string()),
            base_url: Some(summary.base_url),
            selected_model: Some(summary.model),
            api_key_status: "saved",
            can_configure,
        }
    }

    pub(crate) fn missing(can_configure: bool) -> Self {
        Self {
            source: "none",
            detected_protocol: None,
            base_url: None,
            selected_model: None,
            api_key_status: "missing",
            can_configure,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct AgentModelCatalog {
    pub(crate) detected_protocol: String,
    pub(crate) models: Vec<AgentModelCandidate>,
    pub(crate) recommended_model: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct AgentModelCandidate {
    pub(crate) id: String,
    pub(crate) display_name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AgentModelCatalogRequest {
    pub(super) base_url: String,
    pub(super) api_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AgentSettingsUpdateRequest {
    pub(super) base_url: String,
    pub(super) api_key: Option<String>,
    pub(super) selected_model: String,
}

pub(crate) fn decode_agent_model_catalog_request(
    body: &[u8],
) -> Result<AgentModelCatalogRequest, AgentModelSettingsError> {
    decode_request(body)
}

pub(crate) fn decode_agent_settings_update_request(
    body: &[u8],
) -> Result<AgentSettingsUpdateRequest, AgentModelSettingsError> {
    decode_request(body)
}

fn decode_request<T>(body: &[u8]) -> Result<T, AgentModelSettingsError>
where
    T: DeserializeOwned,
{
    serde_json::from_slice(body).map_err(|_| AgentModelSettingsError::InvalidJson)
}

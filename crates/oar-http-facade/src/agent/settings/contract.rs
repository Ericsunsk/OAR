use serde::{de::DeserializeOwned, Deserialize, Serialize};

use super::AgentModelSettingsError;
use crate::agent::AgentProviderConfigSummary;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AgentSettingsSnapshot {
    source: &'static str,
    detected_protocol: Option<String>,
    base_url: Option<String>,
    selected_model: Option<String>,
    api_key_status: &'static str,
    can_configure: bool,
}

impl AgentSettingsSnapshot {
    pub(crate) fn user(
        detected_protocol: String,
        base_url: String,
        selected_model: String,
    ) -> Self {
        Self {
            source: "user",
            detected_protocol: Some(detected_protocol),
            base_url: Some(base_url),
            selected_model: Some(selected_model),
            api_key_status: "saved",
            can_configure: true,
        }
    }

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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn settings_snapshot_constructors_emit_stable_contract_values() {
        let user = serde_json::to_value(AgentSettingsSnapshot::user(
            "openai-compatible".to_string(),
            "https://llm.example.test/v1".to_string(),
            "gpt-4.1".to_string(),
        ))
        .expect("user snapshot json");
        assert_eq!(
            user,
            json!({
                "source": "user",
                "detected_protocol": "openai-compatible",
                "base_url": "https://llm.example.test/v1",
                "selected_model": "gpt-4.1",
                "api_key_status": "saved",
                "can_configure": true
            })
        );

        let env = serde_json::to_value(AgentSettingsSnapshot::from_summary(
            AgentProviderConfigSummary {
                protocol: "anthropic",
                base_url: "https://api.anthropic.com/v1".to_string(),
                model: "claude-sonnet-4-5".to_string(),
            },
            false,
        ))
        .expect("env snapshot json");
        assert_eq!(
            env,
            json!({
                "source": "env",
                "detected_protocol": "anthropic",
                "base_url": "https://api.anthropic.com/v1",
                "selected_model": "claude-sonnet-4-5",
                "api_key_status": "saved",
                "can_configure": false
            })
        );

        let missing = serde_json::to_value(AgentSettingsSnapshot::missing(true))
            .expect("missing snapshot json");
        assert_eq!(
            missing,
            json!({
                "source": "none",
                "detected_protocol": null,
                "base_url": null,
                "selected_model": null,
                "api_key_status": "missing",
                "can_configure": true
            })
        );
    }
}

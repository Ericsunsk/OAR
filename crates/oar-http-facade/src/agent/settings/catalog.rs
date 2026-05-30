use reqwest::Url;
use serde::Deserialize;

use super::base_url::agent_base_url_candidates;
use super::{AgentModelCandidate, AgentModelCatalog, AgentModelSettingsError};
use crate::agent::{agent_endpoint_url, AgentProtocol, DEFAULT_ANTHROPIC_VERSION};

#[derive(Debug, Clone)]
struct ProtocolProbe {
    protocol: AgentProtocol,
    models: Vec<AgentModelCandidate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProbeError {
    Unauthorized,
    Other,
}

#[derive(Debug, Clone)]
pub(super) struct CatalogDetection {
    pub(super) base_url: Url,
    pub(super) catalog: AgentModelCatalog,
}

#[derive(Debug, Deserialize)]
struct ModelListResponseDTO {
    data: Vec<ModelDTO>,
}

#[derive(Debug, Deserialize)]
struct ModelDTO {
    id: String,
    display_name: Option<String>,
}

pub(super) async fn detect_catalog_with_client(
    client: &reqwest::Client,
    base_url: Url,
    api_key: &str,
) -> Result<CatalogDetection, AgentModelSettingsError> {
    let mut unauthorized = false;
    for candidate in agent_base_url_candidates(&base_url) {
        match detect_catalog_for_base_url(client, &candidate, api_key).await {
            Ok(catalog) => {
                return Ok(CatalogDetection {
                    base_url: candidate,
                    catalog,
                });
            }
            Err(AgentModelSettingsError::UpstreamUnauthorized) => unauthorized = true,
            Err(AgentModelSettingsError::DetectionFailed) => {}
            Err(error) => return Err(error),
        }
    }

    if unauthorized {
        Err(AgentModelSettingsError::UpstreamUnauthorized)
    } else {
        Err(AgentModelSettingsError::DetectionFailed)
    }
}

async fn detect_catalog_for_base_url(
    client: &reqwest::Client,
    base_url: &Url,
    api_key: &str,
) -> Result<AgentModelCatalog, AgentModelSettingsError> {
    let mut unauthorized = false;
    for protocol in protocol_probe_order(base_url) {
        match probe_protocol_models(protocol, client, base_url, api_key).await {
            Ok(detected) => return Ok(catalog_from_probe(detected)),
            Err(ProbeError::Unauthorized) => unauthorized = true,
            Err(ProbeError::Other) => {}
        }
    }

    if unauthorized {
        Err(AgentModelSettingsError::UpstreamUnauthorized)
    } else {
        Err(AgentModelSettingsError::DetectionFailed)
    }
}

fn protocol_probe_order(base_url: &Url) -> [AgentProtocol; 2] {
    let host = base_url.host_str().unwrap_or_default();
    if host.contains("anthropic") {
        [AgentProtocol::Anthropic, AgentProtocol::OpenAICompatible]
    } else {
        [AgentProtocol::OpenAICompatible, AgentProtocol::Anthropic]
    }
}

async fn probe_protocol_models(
    protocol: AgentProtocol,
    client: &reqwest::Client,
    base_url: &Url,
    api_key: &str,
) -> Result<ProtocolProbe, ProbeError> {
    match protocol {
        AgentProtocol::OpenAICompatible => probe_openai_models(client, base_url, api_key).await,
        AgentProtocol::Anthropic => probe_anthropic_models(client, base_url, api_key).await,
    }
}

fn catalog_from_probe(detected: ProtocolProbe) -> AgentModelCatalog {
    let recommended_model = recommended_model(detected.protocol, &detected.models);
    AgentModelCatalog {
        detected_protocol: detected.protocol.as_str().to_string(),
        models: detected.models,
        recommended_model,
    }
}

async fn probe_openai_models(
    client: &reqwest::Client,
    base_url: &Url,
    api_key: &str,
) -> Result<ProtocolProbe, ProbeError> {
    let response = client
        .get(agent_endpoint_url(base_url, "models"))
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|_| ProbeError::Other)?;
    models_from_response(response)
        .await
        .map(|models| ProtocolProbe {
            protocol: AgentProtocol::OpenAICompatible,
            models,
        })
}

async fn probe_anthropic_models(
    client: &reqwest::Client,
    base_url: &Url,
    api_key: &str,
) -> Result<ProtocolProbe, ProbeError> {
    let response = client
        .get(agent_endpoint_url(base_url, "models"))
        .header("x-api-key", api_key)
        .header("anthropic-version", DEFAULT_ANTHROPIC_VERSION)
        .send()
        .await
        .map_err(|_| ProbeError::Other)?;
    models_from_response(response)
        .await
        .map(|models| ProtocolProbe {
            protocol: AgentProtocol::Anthropic,
            models,
        })
}

async fn models_from_response(
    response: reqwest::Response,
) -> Result<Vec<AgentModelCandidate>, ProbeError> {
    if matches!(response.status().as_u16(), 401 | 403) {
        return Err(ProbeError::Unauthorized);
    }
    if !response.status().is_success() {
        return Err(ProbeError::Other);
    }
    let dto = response
        .json::<ModelListResponseDTO>()
        .await
        .map_err(|_| ProbeError::Other)?;
    let models = dto
        .data
        .into_iter()
        .filter_map(|model| {
            let id = model.id.trim().to_string();
            if id.is_empty() {
                return None;
            }
            let display_name = model
                .display_name
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| id.clone());
            Some(AgentModelCandidate { id, display_name })
        })
        .collect::<Vec<_>>();
    (!models.is_empty())
        .then_some(models)
        .ok_or(ProbeError::Other)
}

fn recommended_model(protocol: AgentProtocol, models: &[AgentModelCandidate]) -> Option<String> {
    let preferred = match protocol {
        AgentProtocol::OpenAICompatible => ["gpt-4.1", "gpt-4o", "gpt-4"],
        AgentProtocol::Anthropic => ["claude-sonnet-4-5", "claude-3-5-sonnet", "claude-3-sonnet"],
    };
    for candidate in preferred {
        if let Some(model) = models.iter().find(|model| model.id == candidate) {
            return Some(model.id.clone());
        }
    }
    if protocol == AgentProtocol::Anthropic {
        if let Some(model) = models.iter().find(|model| model.id.contains("sonnet")) {
            return Some(model.id.clone());
        }
    }
    models.first().map(|model| model.id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_probe_order_uses_host_hint_or_openai_compatible_default() {
        let anthropic_url = Url::parse("https://api.anthropic.com/v1").expect("url");
        let generic_url = Url::parse("https://llm.example.test/v1").expect("url");

        assert_eq!(
            protocol_probe_order(&anthropic_url),
            [AgentProtocol::Anthropic, AgentProtocol::OpenAICompatible]
        );
        assert_eq!(
            protocol_probe_order(&generic_url),
            [AgentProtocol::OpenAICompatible, AgentProtocol::Anthropic]
        );
    }
}

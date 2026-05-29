use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::Aes256Gcm;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};

use super::{
    agent_http_client, is_allowed_agent_base_url, AgentProtocol, AgentProviderConfig,
    AgentProviderConfigSummary, AgentRuntime, AgentRuntimeConfigError,
};

const ANTHROPIC_VERSION: &str = "2023-06-01";
const SECRET_ENVELOPE_VERSION_V1: u8 = 1;
const SECRET_NONCE_LEN_V1: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentModelSettingsError {
    InvalidJson,
    MissingBaseURL,
    MissingApiKey,
    MissingModel,
    InvalidBaseURL,
    DetectionFailed,
    DetectionAmbiguous,
    UpstreamUnauthorized,
    ModelNotDetected,
    StoreUnavailable,
    SecretCryptoFailed,
    InvalidStoredProtocol,
}

impl std::fmt::Display for AgentModelSettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidJson => write!(f, "agent_settings_invalid_json"),
            Self::MissingBaseURL => write!(f, "agent_settings_base_url_required"),
            Self::MissingApiKey => write!(f, "agent_settings_api_key_required"),
            Self::MissingModel => write!(f, "agent_settings_model_required"),
            Self::InvalidBaseURL => write!(f, "agent_settings_base_url_invalid"),
            Self::DetectionFailed => write!(f, "agent_settings_model_detection_failed"),
            Self::DetectionAmbiguous => write!(f, "agent_settings_protocol_detection_ambiguous"),
            Self::UpstreamUnauthorized => write!(f, "agent_settings_api_key_rejected"),
            Self::ModelNotDetected => write!(f, "agent_settings_model_not_detected"),
            Self::StoreUnavailable => write!(f, "agent_settings_store_unavailable"),
            Self::SecretCryptoFailed => write!(f, "agent_settings_secret_crypto_failed"),
            Self::InvalidStoredProtocol => write!(f, "agent_settings_protocol_invalid"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AgentSettingsSnapshot {
    pub(crate) source: &'static str,
    pub(crate) detected_protocol: Option<String>,
    pub(crate) base_url: Option<String>,
    pub(crate) selected_model: Option<String>,
    pub(crate) api_key_status: &'static str,
    pub(crate) can_configure: bool,
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
    base_url: String,
    api_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AgentSettingsUpdateRequest {
    base_url: String,
    api_key: Option<String>,
    selected_model: String,
}

pub(crate) fn decode_agent_model_catalog_request(
    body: &[u8],
) -> Result<AgentModelCatalogRequest, AgentModelSettingsError> {
    serde_json::from_slice(body).map_err(|_| AgentModelSettingsError::InvalidJson)
}

pub(crate) fn decode_agent_settings_update_request(
    body: &[u8],
) -> Result<AgentSettingsUpdateRequest, AgentModelSettingsError> {
    serde_json::from_slice(body).map_err(|_| AgentModelSettingsError::InvalidJson)
}

#[derive(Clone)]
pub(crate) struct AgentModelSettingsRuntime {
    pool: PgPool,
    key_id: String,
    key_material: [u8; 32],
    client: reqwest::Client,
}

impl std::fmt::Debug for AgentModelSettingsRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentModelSettingsRuntime")
            .field("pool", &"[REDACTED]")
            .field("key_id", &"[REDACTED]")
            .field("key_material", &"[REDACTED]")
            .field("client", &"[REDACTED]")
            .finish()
    }
}

impl AgentModelSettingsRuntime {
    pub(crate) fn new(
        pool: PgPool,
        key_id: String,
        key_material: [u8; 32],
    ) -> Result<Self, AgentRuntimeConfigError> {
        Ok(Self {
            pool,
            key_id,
            key_material,
            client: agent_http_client()?,
        })
    }

    pub(crate) async fn snapshot(
        &self,
        tenant_id: &str,
        user_id: &str,
        default_runtime: Option<&AgentRuntime>,
    ) -> Result<AgentSettingsSnapshot, AgentModelSettingsError> {
        if let Some(setting) = self.load_setting(tenant_id, user_id).await? {
            return Ok(AgentSettingsSnapshot {
                source: "user",
                detected_protocol: Some(setting.protocol.as_str().to_string()),
                base_url: Some(setting.base_url.as_str().to_string()),
                selected_model: Some(setting.selected_model),
                api_key_status: "saved",
                can_configure: true,
            });
        }

        if let Some(default_runtime) = default_runtime {
            return Ok(snapshot_from_summary(default_runtime.config_summary()));
        }

        Ok(AgentSettingsSnapshot {
            source: "none",
            detected_protocol: None,
            base_url: None,
            selected_model: None,
            api_key_status: "missing",
            can_configure: true,
        })
    }

    pub(crate) async fn detect_catalog(
        &self,
        tenant_id: &str,
        user_id: &str,
        request: AgentModelCatalogRequest,
    ) -> Result<AgentModelCatalog, AgentModelSettingsError> {
        let base_url = parse_base_url(&request.base_url)?;
        let api_key = self
            .api_key_for_request(tenant_id, user_id, &base_url, request.api_key)
            .await?;
        detect_catalog_with_client(&self.client, base_url, &api_key)
            .await
            .map(|detection| detection.catalog)
    }

    pub(crate) async fn save_settings(
        &self,
        tenant_id: &str,
        user_id: &str,
        request: AgentSettingsUpdateRequest,
        default_runtime: Option<&AgentRuntime>,
    ) -> Result<AgentSettingsSnapshot, AgentModelSettingsError> {
        let base_url = parse_base_url(&request.base_url)?;
        let api_key = self
            .api_key_for_request(tenant_id, user_id, &base_url, request.api_key)
            .await?;
        let selected_model = required_trimmed(
            request.selected_model,
            AgentModelSettingsError::MissingModel,
        )?;
        let detection = detect_catalog_with_client(&self.client, base_url, &api_key).await?;
        if !detection
            .catalog
            .models
            .iter()
            .any(|model| model.id == selected_model)
        {
            return Err(AgentModelSettingsError::ModelNotDetected);
        }

        let protocol = AgentProtocol::from_str(&detection.catalog.detected_protocol)
            .ok_or(AgentModelSettingsError::InvalidStoredProtocol)?;
        let base_url = detection.base_url;
        let encrypted_api_key = encrypt_secret(&self.key_material, api_key.as_bytes())?;
        let api_key_fingerprint =
            secret_fingerprint(&self.key_material, &self.key_id, api_key.as_bytes());
        let anthropic_version =
            (protocol == AgentProtocol::Anthropic).then(|| ANTHROPIC_VERSION.to_string());
        sqlx::query(
            r#"
            INSERT INTO agent_model_settings (
                tenant_id,
                user_id,
                detected_protocol,
                base_url,
                selected_model,
                encrypted_api_key,
                api_key_key_id,
                api_key_fingerprint,
                anthropic_version
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (tenant_id, user_id) DO UPDATE SET
                detected_protocol = EXCLUDED.detected_protocol,
                base_url = EXCLUDED.base_url,
                selected_model = EXCLUDED.selected_model,
                encrypted_api_key = EXCLUDED.encrypted_api_key,
                api_key_key_id = EXCLUDED.api_key_key_id,
                api_key_fingerprint = EXCLUDED.api_key_fingerprint,
                anthropic_version = EXCLUDED.anthropic_version,
                updated_at = now()
            "#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(protocol.as_str())
        .bind(base_url.as_str())
        .bind(&selected_model)
        .bind(encrypted_api_key)
        .bind(&self.key_id)
        .bind(api_key_fingerprint)
        .bind(anthropic_version)
        .execute(&self.pool)
        .await
        .map_err(|_| AgentModelSettingsError::StoreUnavailable)?;

        self.snapshot(tenant_id, user_id, default_runtime).await
    }

    pub(crate) async fn delete_settings(
        &self,
        tenant_id: &str,
        user_id: &str,
        default_runtime: Option<&AgentRuntime>,
    ) -> Result<AgentSettingsSnapshot, AgentModelSettingsError> {
        sqlx::query(
            r#"
            DELETE FROM agent_model_settings
            WHERE tenant_id = $1 AND user_id = $2
            "#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(|_| AgentModelSettingsError::StoreUnavailable)?;

        self.snapshot(tenant_id, user_id, default_runtime).await
    }

    pub(crate) async fn provider_config_for_user(
        &self,
        tenant_id: &str,
        user_id: &str,
    ) -> Result<Option<AgentProviderConfig>, AgentModelSettingsError> {
        let Some(setting) = self.load_setting(tenant_id, user_id).await? else {
            return Ok(None);
        };
        let api_key = decrypt_secret(&self.key_material, &setting.encrypted_api_key)?;
        Ok(Some(AgentProviderConfig::new(
            setting.protocol,
            setting.base_url,
            api_key,
            setting.selected_model,
            setting.anthropic_version,
        )))
    }

    async fn load_setting(
        &self,
        tenant_id: &str,
        user_id: &str,
    ) -> Result<Option<StoredAgentModelSetting>, AgentModelSettingsError> {
        let row = sqlx::query(
            r#"
            SELECT detected_protocol,
                   base_url,
                   selected_model,
                   encrypted_api_key,
                   api_key_key_id,
                   anthropic_version
            FROM agent_model_settings
            WHERE tenant_id = $1 AND user_id = $2
            "#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| AgentModelSettingsError::StoreUnavailable)?;

        row.map(|row| {
            let protocol: String = row
                .try_get("detected_protocol")
                .map_err(|_| AgentModelSettingsError::StoreUnavailable)?;
            let protocol = AgentProtocol::from_str(&protocol)
                .ok_or(AgentModelSettingsError::InvalidStoredProtocol)?;
            let base_url: String = row
                .try_get("base_url")
                .map_err(|_| AgentModelSettingsError::StoreUnavailable)?;
            let base_url = parse_base_url(&base_url)?;
            let stored_key_id: String = row
                .try_get("api_key_key_id")
                .map_err(|_| AgentModelSettingsError::StoreUnavailable)?;
            if stored_key_id != self.key_id {
                return Err(AgentModelSettingsError::SecretCryptoFailed);
            }
            Ok(StoredAgentModelSetting {
                protocol,
                base_url,
                selected_model: row
                    .try_get("selected_model")
                    .map_err(|_| AgentModelSettingsError::StoreUnavailable)?,
                encrypted_api_key: row
                    .try_get("encrypted_api_key")
                    .map_err(|_| AgentModelSettingsError::StoreUnavailable)?,
                anthropic_version: row
                    .try_get("anthropic_version")
                    .map_err(|_| AgentModelSettingsError::StoreUnavailable)?,
            })
        })
        .transpose()
    }

    async fn api_key_for_request(
        &self,
        tenant_id: &str,
        user_id: &str,
        base_url: &Url,
        api_key: Option<String>,
    ) -> Result<String, AgentModelSettingsError> {
        if let Some(api_key) = optional_trimmed_api_key(api_key)? {
            return Ok(api_key);
        }

        let Some(setting) = self.load_setting(tenant_id, user_id).await? else {
            return Err(AgentModelSettingsError::MissingApiKey);
        };
        if !base_urls_share_detection_candidate(&setting.base_url, base_url) {
            return Err(AgentModelSettingsError::MissingApiKey);
        }

        decrypt_secret(&self.key_material, &setting.encrypted_api_key)
    }
}

#[derive(Debug)]
struct StoredAgentModelSetting {
    protocol: AgentProtocol,
    base_url: Url,
    selected_model: String,
    encrypted_api_key: Vec<u8>,
    anthropic_version: Option<String>,
}

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
struct CatalogDetection {
    base_url: Url,
    catalog: AgentModelCatalog,
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

async fn detect_catalog_with_client(
    client: &reqwest::Client,
    base_url: Url,
    api_key: &str,
) -> Result<CatalogDetection, AgentModelSettingsError> {
    let mut ambiguous = false;
    let mut unauthorized = false;
    for candidate in agent_base_url_candidates(&base_url) {
        match detect_catalog_for_base_url(client, &candidate, api_key).await {
            Ok(catalog) => {
                return Ok(CatalogDetection {
                    base_url: candidate,
                    catalog,
                });
            }
            Err(AgentModelSettingsError::DetectionAmbiguous) => ambiguous = true,
            Err(AgentModelSettingsError::UpstreamUnauthorized) => unauthorized = true,
            Err(AgentModelSettingsError::DetectionFailed) => {}
            Err(error) => return Err(error),
        }
    }

    if unauthorized {
        Err(AgentModelSettingsError::UpstreamUnauthorized)
    } else if ambiguous {
        Err(AgentModelSettingsError::DetectionAmbiguous)
    } else {
        Err(AgentModelSettingsError::DetectionFailed)
    }
}

async fn detect_catalog_for_base_url(
    client: &reqwest::Client,
    base_url: &Url,
    api_key: &str,
) -> Result<AgentModelCatalog, AgentModelSettingsError> {
    let openai = probe_openai_models(client, base_url, api_key);
    let anthropic = probe_anthropic_models(client, base_url, api_key);
    let (openai, anthropic) = tokio::join!(openai, anthropic);
    let unauthorized = matches!(&openai, Err(ProbeError::Unauthorized))
        || matches!(&anthropic, Err(ProbeError::Unauthorized));

    let detected = match (openai.ok(), anthropic.ok()) {
        (Some(openai), None) => openai,
        (None, Some(anthropic)) => anthropic,
        (Some(openai), Some(anthropic)) => choose_ambiguous_protocol(base_url, openai, anthropic)?,
        (None, None) if unauthorized => return Err(AgentModelSettingsError::UpstreamUnauthorized),
        (None, None) => return Err(AgentModelSettingsError::DetectionFailed),
    };
    let recommended_model = recommended_model(detected.protocol, &detected.models);
    Ok(AgentModelCatalog {
        detected_protocol: detected.protocol.as_str().to_string(),
        models: detected.models,
        recommended_model,
    })
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
        .header("anthropic-version", ANTHROPIC_VERSION)
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

fn choose_ambiguous_protocol(
    base_url: &Url,
    openai: ProtocolProbe,
    anthropic: ProtocolProbe,
) -> Result<ProtocolProbe, AgentModelSettingsError> {
    let host = base_url.host_str().unwrap_or_default();
    if host.contains("anthropic") {
        return Ok(anthropic);
    }
    if host.contains("openai") {
        return Ok(openai);
    }
    Err(AgentModelSettingsError::DetectionAmbiguous)
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

fn parse_base_url(value: &str) -> Result<Url, AgentModelSettingsError> {
    let base_url = required_trimmed(value.to_string(), AgentModelSettingsError::MissingBaseURL)?;
    let mut base_url = Url::parse(&base_url)
        .ok()
        .filter(is_allowed_agent_base_url)
        .ok_or(AgentModelSettingsError::InvalidBaseURL)?;
    base_url.set_query(None);
    base_url.set_fragment(None);
    Ok(base_url)
}

fn required_trimmed(
    value: String,
    missing: AgentModelSettingsError,
) -> Result<String, AgentModelSettingsError> {
    let value = value.trim();
    if value.is_empty() {
        Err(missing)
    } else {
        Ok(value.to_string())
    }
}

fn optional_trimmed_api_key(
    value: Option<String>,
) -> Result<Option<String>, AgentModelSettingsError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }

    let mut parts = value.splitn(2, char::is_whitespace);
    if parts
        .next()
        .map(|scheme| scheme.eq_ignore_ascii_case("bearer"))
        .unwrap_or(false)
    {
        let value = parts
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or(AgentModelSettingsError::MissingApiKey)?;
        return Ok(Some(value.to_string()));
    }

    Ok(Some(value.to_string()))
}

fn agent_base_url_candidates(base_url: &Url) -> Vec<Url> {
    let mut candidates = Vec::new();
    if let Some(trimmed) = trim_known_endpoint_suffix(base_url) {
        push_api_base_url_candidates(&mut candidates, trimmed);
    }

    push_api_base_url_candidates(&mut candidates, base_url.clone());
    candidates
}

fn base_urls_share_detection_candidate(left: &Url, right: &Url) -> bool {
    left == right
        || agent_base_url_candidates(left)
            .iter()
            .any(|candidate| candidate == right)
        || agent_base_url_candidates(right)
            .iter()
            .any(|candidate| candidate == left)
}

fn push_api_base_url_candidates(candidates: &mut Vec<Url>, base_url: Url) {
    let is_root = is_root_path(&base_url);
    let prefer_versioned = is_root && prefers_versioned_api_base(&base_url);
    if prefer_versioned {
        push_versioned_api_base_url(candidates, &base_url);
    }

    push_unique_url(candidates, base_url.clone());

    if is_root && !prefer_versioned {
        push_versioned_api_base_url(candidates, &base_url);
    }
}

fn trim_known_endpoint_suffix(base_url: &Url) -> Option<Url> {
    let path = base_url.path().trim_end_matches('/');
    for suffix in ["/chat/completions", "/messages", "/models"] {
        if let Some(prefix) = path.strip_suffix(suffix) {
            let mut trimmed = base_url.clone();
            trimmed.set_path(if prefix.is_empty() { "/" } else { prefix });
            trimmed.set_query(None);
            trimmed.set_fragment(None);
            return Some(trimmed);
        }
    }
    None
}

fn push_versioned_api_base_url(candidates: &mut Vec<Url>, base_url: &Url) {
    let mut versioned = base_url.clone();
    versioned.set_path("/v1");
    versioned.set_query(None);
    versioned.set_fragment(None);
    push_unique_url(candidates, versioned);
}

fn is_root_path(base_url: &Url) -> bool {
    matches!(base_url.path(), "" | "/")
}

fn prefers_versioned_api_base(base_url: &Url) -> bool {
    base_url
        .host_str()
        .map(|host| host.contains("openai") || host.contains("anthropic"))
        .unwrap_or(false)
}

fn push_unique_url(urls: &mut Vec<Url>, url: Url) {
    if !urls.iter().any(|existing| existing == &url) {
        urls.push(url);
    }
}

fn agent_endpoint_url(base_url: &Url, suffix: &str) -> Url {
    let mut endpoint = base_url.clone();
    let path = format!("{}/{}", endpoint.path().trim_end_matches('/'), suffix);
    endpoint.set_path(&path);
    endpoint.set_query(None);
    endpoint.set_fragment(None);
    endpoint
}

fn snapshot_from_summary(summary: AgentProviderConfigSummary) -> AgentSettingsSnapshot {
    AgentSettingsSnapshot {
        source: "env",
        detected_protocol: Some(summary.protocol.to_string()),
        base_url: Some(summary.base_url),
        selected_model: Some(summary.model),
        api_key_status: "saved",
        can_configure: true,
    }
}

fn encrypt_secret(
    key_material: &[u8; 32],
    plaintext: &[u8],
) -> Result<Vec<u8>, AgentModelSettingsError> {
    let aead = Aes256Gcm::new_from_slice(key_material)
        .map_err(|_| AgentModelSettingsError::SecretCryptoFailed)?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = aead
        .encrypt(&nonce, plaintext)
        .map_err(|_| AgentModelSettingsError::SecretCryptoFailed)?;
    let mut envelope = Vec::with_capacity(2 + nonce.len() + ciphertext.len());
    envelope.push(SECRET_ENVELOPE_VERSION_V1);
    envelope.push(SECRET_NONCE_LEN_V1 as u8);
    envelope.extend_from_slice(&nonce);
    envelope.extend_from_slice(&ciphertext);
    Ok(envelope)
}

fn decrypt_secret(
    key_material: &[u8; 32],
    envelope: &[u8],
) -> Result<String, AgentModelSettingsError> {
    if envelope.len() < 2 + SECRET_NONCE_LEN_V1
        || envelope[0] != SECRET_ENVELOPE_VERSION_V1
        || envelope[1] as usize != SECRET_NONCE_LEN_V1
    {
        return Err(AgentModelSettingsError::SecretCryptoFailed);
    }
    let nonce = &envelope[2..(2 + SECRET_NONCE_LEN_V1)];
    let ciphertext = &envelope[(2 + SECRET_NONCE_LEN_V1)..];
    if ciphertext.is_empty() {
        return Err(AgentModelSettingsError::SecretCryptoFailed);
    }
    let aead = Aes256Gcm::new_from_slice(key_material)
        .map_err(|_| AgentModelSettingsError::SecretCryptoFailed)?;
    let plaintext = aead
        .decrypt(nonce.into(), ciphertext)
        .map_err(|_| AgentModelSettingsError::SecretCryptoFailed)?;
    String::from_utf8(plaintext).map_err(|_| AgentModelSettingsError::SecretCryptoFailed)
}

fn secret_fingerprint(key_material: &[u8; 32], key_id: &str, secret: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key_id.as_bytes());
    hasher.update(key_material);
    hasher.update(secret);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_envelope_roundtrips_without_plaintext() {
        let key = [9; 32];
        let encrypted = encrypt_secret(&key, b"sk-sensitive").expect("encrypt");

        assert!(!encrypted
            .windows(b"sk-sensitive".len())
            .any(|w| w == b"sk-sensitive"));
        assert_eq!(
            decrypt_secret(&key, &encrypted).expect("decrypt"),
            "sk-sensitive"
        );
        assert_eq!(
            secret_fingerprint(&key, "key-test", b"sk-sensitive"),
            secret_fingerprint(&key, "key-test", b"sk-sensitive")
        );
        assert_ne!(
            secret_fingerprint(&key, "key-test", b"sk-sensitive"),
            secret_fingerprint(&key, "key-test", b"sk-other")
        );
    }

    #[test]
    fn ambiguous_protocol_uses_host_hint_or_fails_closed() {
        let openai = ProtocolProbe {
            protocol: AgentProtocol::OpenAICompatible,
            models: vec![AgentModelCandidate {
                id: "gpt-4.1".to_string(),
                display_name: "gpt-4.1".to_string(),
            }],
        };
        let anthropic = ProtocolProbe {
            protocol: AgentProtocol::Anthropic,
            models: vec![AgentModelCandidate {
                id: "claude-sonnet-4-5".to_string(),
                display_name: "claude-sonnet-4-5".to_string(),
            }],
        };

        let anthropic_url = Url::parse("https://api.anthropic.com/v1").expect("url");
        let generic_url = Url::parse("https://llm.example.test/v1").expect("url");

        assert_eq!(
            choose_ambiguous_protocol(&anthropic_url, openai.clone(), anthropic.clone())
                .expect("host hint")
                .protocol,
            AgentProtocol::Anthropic
        );
        assert_eq!(
            choose_ambiguous_protocol(&generic_url, openai, anthropic).expect_err("ambiguous"),
            AgentModelSettingsError::DetectionAmbiguous
        );
    }

    #[test]
    fn model_detection_base_url_candidates_normalize_common_inputs() {
        let openai_root = Url::parse("https://api.openai.com").expect("url");
        let openai_candidates = agent_base_url_candidates(&openai_root)
            .into_iter()
            .map(|url| url.as_str().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            openai_candidates,
            vec!["https://api.openai.com/v1", "https://api.openai.com/"]
        );

        let chat_completions =
            Url::parse("https://api.openai.com/v1/chat/completions").expect("url");
        let chat_candidates = agent_base_url_candidates(&chat_completions)
            .into_iter()
            .map(|url| url.as_str().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            chat_candidates,
            vec![
                "https://api.openai.com/v1",
                "https://api.openai.com/v1/chat/completions"
            ]
        );

        let generic_root = Url::parse("https://llm.example.test").expect("url");
        let generic_candidates = agent_base_url_candidates(&generic_root)
            .into_iter()
            .map(|url| url.as_str().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            generic_candidates,
            vec!["https://llm.example.test/", "https://llm.example.test/v1"]
        );
    }

    #[test]
    fn saved_api_key_base_url_comparison_accepts_detection_equivalents() {
        let root = Url::parse("https://llm.example.test").expect("url");
        let versioned = Url::parse("https://llm.example.test/v1").expect("url");
        let other = Url::parse("https://other.example.test/v1").expect("url");

        assert!(base_urls_share_detection_candidate(&root, &versioned));
        assert!(base_urls_share_detection_candidate(&versioned, &root));
        assert!(!base_urls_share_detection_candidate(&versioned, &other));
    }

    #[test]
    fn parsed_base_url_drops_query_and_fragment() {
        let base_url =
            parse_base_url("https://llm.example.test/v1/models?foo=bar#frag").expect("url");

        assert_eq!(base_url.as_str(), "https://llm.example.test/v1/models");
    }

    #[test]
    fn optional_api_key_accepts_plain_or_bearer_pasted_values() {
        assert_eq!(
            optional_trimmed_api_key(Some("  sk-test  ".to_string())).expect("api key"),
            Some("sk-test".to_string())
        );
        assert_eq!(
            optional_trimmed_api_key(Some("Bearer sk-test".to_string())).expect("bearer api key"),
            Some("sk-test".to_string())
        );
        assert_eq!(
            optional_trimmed_api_key(Some("bearer   sk-test  ".to_string()))
                .expect("lowercase bearer api key"),
            Some("sk-test".to_string())
        );
        assert_eq!(
            optional_trimmed_api_key(Some("   ".to_string())).expect("empty api key"),
            None
        );
        assert_eq!(
            optional_trimmed_api_key(Some("Bearer   ".to_string())).expect_err("missing api key"),
            AgentModelSettingsError::MissingApiKey
        );
    }
}

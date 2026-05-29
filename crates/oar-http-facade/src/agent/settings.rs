use reqwest::Url;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};

use super::{
    agent_http_client, AgentProtocol, AgentProviderConfig, AgentProviderConfigSummary,
    AgentRuntime, AgentRuntimeConfigError, DEFAULT_ANTHROPIC_VERSION,
};

mod base_url;
mod catalog;
mod secret;

use base_url::{
    base_urls_share_detection_candidate, optional_trimmed_api_key, parse_base_url, required_trimmed,
};
use catalog::detect_catalog_with_client;
use secret::{decrypt_secret, encrypt_secret, secret_fingerprint};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentModelSettingsError {
    InvalidJson,
    MissingBaseURL,
    MissingApiKey,
    MissingModel,
    InvalidBaseURL,
    DetectionFailed,
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
            (protocol == AgentProtocol::Anthropic).then(|| DEFAULT_ANTHROPIC_VERSION.to_string());
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

use reqwest::Url;
use sqlx::PgPool;

use super::base_url::{
    base_urls_share_detection_candidate, optional_trimmed_api_key, parse_base_url, required_trimmed,
};
use super::catalog::{detect_catalog_with_client, CatalogDetection};
use super::contract::{
    AgentModelCatalog, AgentModelCatalogRequest, AgentSettingsSnapshot, AgentSettingsUpdateRequest,
};
use super::secret::{decrypt_secret, encrypt_secret, secret_fingerprint};
use super::store::{
    self, delete_setting, upsert_setting, AgentModelSettingUpsert, StoredAgentModelSetting,
};
use super::AgentModelSettingsError;
use crate::agent::{
    agent_http_client, AgentProtocol, AgentProviderConfig, AgentRuntime, AgentRuntimeConfigError,
    DEFAULT_ANTHROPIC_VERSION,
};

#[derive(Clone)]
pub(crate) struct AgentModelSettingsRuntime {
    pool: PgPool,
    key_id: String,
    key_material: [u8; 32],
    client: reqwest::Client,
}

struct ResolvedCatalogDetection {
    detection: CatalogDetection,
    api_key: String,
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
            return Ok(AgentSettingsSnapshot::user(
                setting.protocol.as_str().to_string(),
                setting.base_url.as_str().to_string(),
                setting.selected_model,
            ));
        }

        if let Some(default_runtime) = default_runtime {
            return Ok(AgentSettingsSnapshot::from_summary(
                default_runtime.config_summary(),
                true,
            ));
        }

        Ok(AgentSettingsSnapshot::missing(true))
    }

    pub(crate) async fn detect_catalog(
        &self,
        tenant_id: &str,
        user_id: &str,
        request: AgentModelCatalogRequest,
    ) -> Result<AgentModelCatalog, AgentModelSettingsError> {
        let AgentModelCatalogRequest { base_url, api_key } = request;
        self.detect_catalog_for_request(tenant_id, user_id, &base_url, api_key)
            .await
            .map(|resolved| resolved.detection.catalog)
    }

    pub(crate) async fn save_settings(
        &self,
        tenant_id: &str,
        user_id: &str,
        request: AgentSettingsUpdateRequest,
        default_runtime: Option<&AgentRuntime>,
    ) -> Result<AgentSettingsSnapshot, AgentModelSettingsError> {
        let AgentSettingsUpdateRequest {
            base_url,
            api_key,
            selected_model,
        } = request;
        let selected_model =
            required_trimmed(selected_model, AgentModelSettingsError::MissingModel)?;
        let resolved = self
            .detect_catalog_for_request(tenant_id, user_id, &base_url, api_key)
            .await?;
        let detection = resolved.detection;
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
        let encrypted_api_key = encrypt_secret(&self.key_material, resolved.api_key.as_bytes())?;
        let api_key_fingerprint = secret_fingerprint(
            &self.key_material,
            &self.key_id,
            resolved.api_key.as_bytes(),
        );
        let anthropic_version =
            (protocol == AgentProtocol::Anthropic).then(|| DEFAULT_ANTHROPIC_VERSION.to_string());
        upsert_setting(
            &self.pool,
            tenant_id,
            user_id,
            AgentModelSettingUpsert {
                protocol,
                base_url,
                selected_model,
                encrypted_api_key,
                api_key_key_id: self.key_id.clone(),
                api_key_fingerprint,
                anthropic_version,
            },
        )
        .await?;

        self.snapshot(tenant_id, user_id, default_runtime).await
    }

    pub(crate) async fn delete_settings(
        &self,
        tenant_id: &str,
        user_id: &str,
        default_runtime: Option<&AgentRuntime>,
    ) -> Result<AgentSettingsSnapshot, AgentModelSettingsError> {
        delete_setting(&self.pool, tenant_id, user_id).await?;

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
        store::load_setting(&self.pool, &self.key_id, tenant_id, user_id).await
    }

    async fn detect_catalog_for_request(
        &self,
        tenant_id: &str,
        user_id: &str,
        base_url: &str,
        api_key: Option<String>,
    ) -> Result<ResolvedCatalogDetection, AgentModelSettingsError> {
        let base_url = parse_base_url(base_url)?;
        let api_key = self
            .api_key_for_request(tenant_id, user_id, &base_url, api_key)
            .await?;
        let detection = detect_catalog_with_client(&self.client, base_url, &api_key).await?;

        Ok(ResolvedCatalogDetection { detection, api_key })
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

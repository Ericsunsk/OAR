use reqwest::Url;
use sqlx::{PgPool, Row};

use super::{base_url::parse_base_url, AgentModelSettingsError};
use crate::agent::AgentProtocol;

#[derive(Debug)]
pub(super) struct StoredAgentModelSetting {
    pub(super) protocol: AgentProtocol,
    pub(super) base_url: Url,
    pub(super) selected_model: String,
    pub(super) encrypted_api_key: Vec<u8>,
    pub(super) anthropic_version: Option<String>,
}

#[derive(Debug)]
pub(super) struct AgentModelSettingUpsert {
    pub(super) protocol: AgentProtocol,
    pub(super) base_url: Url,
    pub(super) selected_model: String,
    pub(super) encrypted_api_key: Vec<u8>,
    pub(super) api_key_key_id: String,
    pub(super) api_key_fingerprint: String,
    pub(super) anthropic_version: Option<String>,
}

pub(super) async fn load_setting(
    pool: &PgPool,
    key_id: &str,
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
    .fetch_optional(pool)
    .await
    .map_err(|_| AgentModelSettingsError::StoreUnavailable)?;

    row.map(|row| map_setting_row(row, key_id)).transpose()
}

pub(super) async fn upsert_setting(
    pool: &PgPool,
    tenant_id: &str,
    user_id: &str,
    setting: AgentModelSettingUpsert,
) -> Result<(), AgentModelSettingsError> {
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
    .bind(setting.protocol.as_str())
    .bind(setting.base_url.as_str())
    .bind(setting.selected_model)
    .bind(setting.encrypted_api_key)
    .bind(setting.api_key_key_id)
    .bind(setting.api_key_fingerprint)
    .bind(setting.anthropic_version)
    .execute(pool)
    .await
    .map_err(|_| AgentModelSettingsError::StoreUnavailable)?;

    Ok(())
}

pub(super) async fn delete_setting(
    pool: &PgPool,
    tenant_id: &str,
    user_id: &str,
) -> Result<(), AgentModelSettingsError> {
    sqlx::query(
        r#"
        DELETE FROM agent_model_settings
        WHERE tenant_id = $1 AND user_id = $2
        "#,
    )
    .bind(tenant_id)
    .bind(user_id)
    .execute(pool)
    .await
    .map_err(|_| AgentModelSettingsError::StoreUnavailable)?;

    Ok(())
}

fn map_setting_row(
    row: sqlx::postgres::PgRow,
    key_id: &str,
) -> Result<StoredAgentModelSetting, AgentModelSettingsError> {
    let protocol: String = row
        .try_get("detected_protocol")
        .map_err(|_| AgentModelSettingsError::StoreUnavailable)?;
    let protocol =
        AgentProtocol::from_str(&protocol).ok_or(AgentModelSettingsError::InvalidStoredProtocol)?;
    let base_url: String = row
        .try_get("base_url")
        .map_err(|_| AgentModelSettingsError::StoreUnavailable)?;
    let base_url = parse_base_url(&base_url)?;
    let stored_key_id: String = row
        .try_get("api_key_key_id")
        .map_err(|_| AgentModelSettingsError::StoreUnavailable)?;
    if stored_key_id != key_id {
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
}

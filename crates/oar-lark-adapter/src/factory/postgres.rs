use sqlx::PgPool;

use crate::config::FeishuOpenApiConfig;
use crate::credentials::StaticFeishuAppCredentialProvider;
use crate::crypto::{AesGcmGrantEncryptor, SystemGrantClock};
use crate::material::FeishuStoredRefreshMaterialProvider;
use crate::oauth::{HttpClientFailure, ReqwestAsyncHttpClient};
use crate::postgres::PostgresFeishuGrantMaterialStore;
use crate::redaction::SecretString;

use super::{
    build_feishu_auth_refresh_adapter, FeishuAuthRefreshAdapter,
    FeishuAuthRefreshAdapterBuildError, StaticAesGcmKeyResolver,
};

pub type PostgresFeishuAuthRefreshMaterialProvider = FeishuStoredRefreshMaterialProvider<
    PostgresFeishuGrantMaterialStore,
    StaticAesGcmKeyResolver,
    StaticFeishuAppCredentialProvider,
>;

pub type PostgresFeishuAuthRefreshAdapter<H> = FeishuAuthRefreshAdapter<
    PostgresFeishuAuthRefreshMaterialProvider,
    AesGcmGrantEncryptor<SystemGrantClock>,
    H,
>;

pub type PostgresAsyncFeishuAuthRefreshAdapter =
    PostgresFeishuAuthRefreshAdapter<ReqwestAsyncHttpClient>;

pub fn build_postgres_async_feishu_auth_refresh_adapter(
    pool: PgPool,
    config: FeishuOpenApiConfig,
    client_id: impl Into<String>,
    client_secret: SecretString,
    grant_key_id: impl Into<String>,
    grant_key_material: [u8; 32],
) -> Result<PostgresAsyncFeishuAuthRefreshAdapter, FeishuAuthRefreshAdapterBuildError> {
    let client_id =
        validate_required_value(client_id, FeishuAuthRefreshAdapterBuildError::EmptyClientId)?;
    let grant_key_id = validate_required_value(
        grant_key_id,
        FeishuAuthRefreshAdapterBuildError::EmptyGrantKeyId,
    )?;
    let http_client =
        ReqwestAsyncHttpClient::with_config(&config).map_err(|_err: HttpClientFailure| {
            FeishuAuthRefreshAdapterBuildError::HttpClientBuildFailed
        })?;
    build_postgres_feishu_auth_refresh_adapter_with_http(
        pool,
        config,
        client_id,
        client_secret,
        grant_key_id,
        grant_key_material,
        http_client,
    )
}

pub fn build_postgres_feishu_auth_refresh_adapter_with_http<H>(
    pool: PgPool,
    config: FeishuOpenApiConfig,
    client_id: impl Into<String>,
    client_secret: SecretString,
    grant_key_id: impl Into<String>,
    grant_key_material: [u8; 32],
    http_client: H,
) -> Result<PostgresFeishuAuthRefreshAdapter<H>, FeishuAuthRefreshAdapterBuildError> {
    let client_id =
        validate_required_value(client_id, FeishuAuthRefreshAdapterBuildError::EmptyClientId)?;
    let grant_key_id = validate_required_value(
        grant_key_id,
        FeishuAuthRefreshAdapterBuildError::EmptyGrantKeyId,
    )?;

    let material_provider = FeishuStoredRefreshMaterialProvider::new(
        PostgresFeishuGrantMaterialStore::new(pool),
        StaticAesGcmKeyResolver::new(grant_key_id.clone(), grant_key_material),
        StaticFeishuAppCredentialProvider::new(client_id, client_secret),
    );
    let encryptor = AesGcmGrantEncryptor::new(grant_key_id, grant_key_material);
    build_feishu_auth_refresh_adapter(config, material_provider, encryptor, http_client)
}

fn validate_required_value(
    value: impl Into<String>,
    error: FeishuAuthRefreshAdapterBuildError,
) -> Result<String, FeishuAuthRefreshAdapterBuildError> {
    let value = value.into();
    if value.trim().is_empty() {
        return Err(error);
    }
    Ok(value)
}

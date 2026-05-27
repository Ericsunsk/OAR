use std::fmt;

use oar_core::lark::auth::adapter::FeishuAuthRefreshAdapter as CoreFeishuAuthRefreshAdapter;
use oar_core::lark::auth::client::{
    FeishuAuthRefreshSafeClient, FeishuAuthRefreshSafeClientConfig,
};
#[cfg(feature = "postgres")]
use sqlx::PgPool;

use crate::config::{FeishuOpenApiConfig, FeishuOpenApiConfigError};
#[cfg(feature = "postgres")]
use crate::credentials::StaticFeishuAppCredentialProvider;
#[cfg(feature = "postgres")]
use crate::crypto::{AesGcmGrantEncryptor, SystemGrantClock};
use crate::material::AesGcmKeyResolver;
#[cfg(feature = "postgres")]
use crate::material::FeishuStoredRefreshMaterialProvider;
use crate::oauth::{
    FeishuOAuthTransport, HttpClientFailure, ReqwestAsyncHttpClient, ReqwestBlockingHttpClient,
};
#[cfg(feature = "postgres")]
use crate::postgres::PostgresFeishuGrantMaterialStore;
#[cfg(feature = "postgres")]
use crate::redaction::SecretString;

pub type FeishuAuthRefreshAdapter<P, E, H> =
    CoreFeishuAuthRefreshAdapter<FeishuAuthRefreshSafeClient<FeishuOAuthTransport<P, E, H>>>;

#[cfg(feature = "postgres")]
pub type PostgresFeishuAuthRefreshMaterialProvider = FeishuStoredRefreshMaterialProvider<
    PostgresFeishuGrantMaterialStore,
    StaticAesGcmKeyResolver,
    StaticFeishuAppCredentialProvider,
>;

#[cfg(feature = "postgres")]
pub type PostgresFeishuAuthRefreshAdapter<H> = FeishuAuthRefreshAdapter<
    PostgresFeishuAuthRefreshMaterialProvider,
    AesGcmGrantEncryptor<SystemGrantClock>,
    H,
>;

#[cfg(feature = "postgres")]
pub type PostgresAsyncFeishuAuthRefreshAdapter =
    PostgresFeishuAuthRefreshAdapter<ReqwestAsyncHttpClient>;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FeishuAuthRefreshAdapterBuildError {
    InvalidConfig(FeishuOpenApiConfigError),
    HttpClientBuildFailed,
    EmptyClientId,
    EmptyGrantKeyId,
}

impl fmt::Debug for FeishuAuthRefreshAdapterBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FeishuAuthRefreshAdapterBuildError::InvalidConfig(_) => {
                write!(f, "FeishuAuthRefreshAdapterBuildError(invalid_config)")
            }
            FeishuAuthRefreshAdapterBuildError::HttpClientBuildFailed => write!(
                f,
                "FeishuAuthRefreshAdapterBuildError(reqwest_client_build_failed)"
            ),
            FeishuAuthRefreshAdapterBuildError::EmptyClientId => {
                write!(f, "FeishuAuthRefreshAdapterBuildError(empty_client_id)")
            }
            FeishuAuthRefreshAdapterBuildError::EmptyGrantKeyId => {
                write!(f, "FeishuAuthRefreshAdapterBuildError(empty_grant_key_id)")
            }
        }
    }
}

impl fmt::Display for FeishuAuthRefreshAdapterBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FeishuAuthRefreshAdapterBuildError::InvalidConfig(err) => {
                write!(f, "feishu auth refresh adapter invalid config: {err}")
            }
            FeishuAuthRefreshAdapterBuildError::HttpClientBuildFailed => {
                write!(f, "feishu auth refresh adapter build failed")
            }
            FeishuAuthRefreshAdapterBuildError::EmptyClientId => {
                write!(f, "feishu client id is required")
            }
            FeishuAuthRefreshAdapterBuildError::EmptyGrantKeyId => {
                write!(f, "feishu grant key id is required")
            }
        }
    }
}

impl std::error::Error for FeishuAuthRefreshAdapterBuildError {}

pub fn build_feishu_auth_refresh_adapter<P, E, H>(
    config: FeishuOpenApiConfig,
    material_provider: P,
    encryptor: E,
    http_client: H,
) -> Result<FeishuAuthRefreshAdapter<P, E, H>, FeishuAuthRefreshAdapterBuildError> {
    config
        .validate()
        .map_err(FeishuAuthRefreshAdapterBuildError::InvalidConfig)?;
    let safe_client_config = FeishuAuthRefreshSafeClientConfig {
        max_response_bytes: config.max_response_bytes,
    };
    let transport = FeishuOAuthTransport::new(config, material_provider, encryptor, http_client);
    let safe_client = FeishuAuthRefreshSafeClient::with_config(transport, safe_client_config);
    Ok(FeishuAuthRefreshAdapter::new(safe_client))
}

pub fn build_reqwest_feishu_auth_refresh_adapter<P, E>(
    config: FeishuOpenApiConfig,
    material_provider: P,
    encryptor: E,
) -> Result<
    FeishuAuthRefreshAdapter<P, E, ReqwestBlockingHttpClient>,
    FeishuAuthRefreshAdapterBuildError,
> {
    let http_client =
        ReqwestBlockingHttpClient::with_config(&config).map_err(|_err: HttpClientFailure| {
            FeishuAuthRefreshAdapterBuildError::HttpClientBuildFailed
        })?;
    build_feishu_auth_refresh_adapter(config, material_provider, encryptor, http_client)
}

pub fn build_async_reqwest_feishu_auth_refresh_adapter<P, E>(
    config: FeishuOpenApiConfig,
    material_provider: P,
    encryptor: E,
) -> Result<
    FeishuAuthRefreshAdapter<P, E, ReqwestAsyncHttpClient>,
    FeishuAuthRefreshAdapterBuildError,
> {
    let http_client =
        ReqwestAsyncHttpClient::with_config(&config).map_err(|_err: HttpClientFailure| {
            FeishuAuthRefreshAdapterBuildError::HttpClientBuildFailed
        })?;
    build_feishu_auth_refresh_adapter(config, material_provider, encryptor, http_client)
}

#[cfg(feature = "postgres")]
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

#[cfg(feature = "postgres")]
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

#[cfg(feature = "postgres")]
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

#[derive(Clone)]
pub struct StaticAesGcmKeyResolver {
    key_id: String,
    key_material: [u8; 32],
}

impl StaticAesGcmKeyResolver {
    pub fn new(key_id: impl Into<String>, key_material: [u8; 32]) -> Self {
        Self {
            key_id: key_id.into(),
            key_material,
        }
    }
}

impl fmt::Debug for StaticAesGcmKeyResolver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StaticAesGcmKeyResolver")
            .field("key_id", &"[REDACTED]")
            .field("key_material", &"[REDACTED]")
            .finish()
    }
}

impl fmt::Display for StaticAesGcmKeyResolver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StaticAesGcmKeyResolver([REDACTED])")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("grant key is unavailable")]
pub struct StaticAesGcmKeyResolverError;

impl AesGcmKeyResolver for StaticAesGcmKeyResolver {
    type Error = StaticAesGcmKeyResolverError;

    fn key_for(&mut self, key_id: &str) -> Result<[u8; 32], Self::Error> {
        if key_id == self.key_id {
            Ok(self.key_material)
        } else {
            Err(StaticAesGcmKeyResolverError)
        }
    }
}

#[cfg(test)]
mod tests;

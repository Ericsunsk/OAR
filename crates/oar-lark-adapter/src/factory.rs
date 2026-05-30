use std::fmt;

use oar_core::lark::auth::adapter::FeishuAuthRefreshAdapter as CoreFeishuAuthRefreshAdapter;
use oar_core::lark::auth::client::{
    FeishuAuthRefreshSafeClient, FeishuAuthRefreshSafeClientConfig,
};

use crate::config::{FeishuOpenApiConfig, FeishuOpenApiConfigError};
use crate::oauth::{
    FeishuOAuthTransport, HttpClientFailure, ReqwestAsyncHttpClient, ReqwestBlockingHttpClient,
};

mod env;
mod key_resolver;
#[cfg(feature = "postgres")]
mod postgres;

pub use env::{PostgresFeishuAuthRefreshEnvConfig, PostgresFeishuAuthRefreshEnvConfigError};
pub use key_resolver::{StaticAesGcmKeyResolver, StaticAesGcmKeyResolverError};
#[cfg(feature = "postgres")]
pub use postgres::{
    build_postgres_async_feishu_auth_refresh_adapter,
    build_postgres_feishu_auth_refresh_adapter_with_http, PostgresAsyncFeishuAuthRefreshAdapter,
    PostgresFeishuAuthRefreshAdapter, PostgresFeishuAuthRefreshMaterialProvider,
};

pub type FeishuAuthRefreshAdapter<P, E, H> =
    CoreFeishuAuthRefreshAdapter<FeishuAuthRefreshSafeClient<FeishuOAuthTransport<P, E, H>>>;

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

#[cfg(test)]
mod tests;

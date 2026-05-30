use crate::{
    build_async_reqwest_feishu_auth_refresh_adapter, build_feishu_auth_refresh_adapter,
    build_reqwest_feishu_auth_refresh_adapter, AesGcmKeyResolver, FeishuOpenApiConfig,
    HttpResponse, PostgresFeishuAuthRefreshEnvConfig, PostgresFeishuAuthRefreshEnvConfigError,
    StaticAesGcmKeyResolver, StaticAesGcmKeyResolverError,
};
#[cfg(feature = "postgres")]
use crate::{
    build_postgres_async_feishu_auth_refresh_adapter,
    build_postgres_feishu_auth_refresh_adapter_with_http, FeishuAuthRefreshAdapterBuildError,
    SecretString,
};
#[cfg(feature = "postgres")]
use oar_core::domain::token_refresh::service::AsyncAuthRefreshAdapter;
use oar_core::domain::token_refresh::service::AuthRefreshAdapter;
use oar_core::domain::token_refresh::types::RefreshOutcome;

mod adapter;
mod env_config;
#[cfg(feature = "postgres")]
mod postgres;
mod support;

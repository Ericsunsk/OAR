#![forbid(unsafe_code)]

pub mod config;
pub mod credentials;
pub mod crypto;
pub mod error;
pub mod factory;
pub mod material;
pub mod oauth;
#[cfg(feature = "postgres")]
pub mod postgres;
pub mod redaction;

pub use config::FeishuOpenApiConfig;
pub use credentials::{
    AsyncFeishuAppCredentialProvider, FeishuAppCredential, FeishuAppCredentialProvider,
    StaticFeishuAppCredentialProvider,
};
pub use crypto::{
    AesGcmGrantEncryptor, AesGcmGrantEncryptorError, GrantTimeSource, SystemGrantClock,
};
pub use error::{
    classify_feishu_refresh_failure, safe_error_for_failure_class, FeishuRefreshFailureClass,
};
pub use factory::{
    build_async_reqwest_feishu_auth_refresh_adapter, build_feishu_auth_refresh_adapter,
    build_reqwest_feishu_auth_refresh_adapter, FeishuAuthRefreshAdapter,
    FeishuAuthRefreshAdapterBuildError,
};
pub use material::{
    AesGcmKeyResolver, AesGcmRefreshMaterialProvider, AesGcmRefreshMaterialProviderError,
    AsyncAesGcmKeyResolver, AsyncFeishuGrantMaterialStore, DecryptedFeishuGrantMaterial,
    FeishuGrantMaterialStore, FeishuStoredRefreshMaterialProvider,
    FeishuStoredRefreshMaterialProviderError, StoredFeishuGrantMaterial,
};
pub use oauth::{
    AsyncFeishuRefreshMaterialProvider, AsyncHttpClient, FeishuGrantEncryptionInput,
    FeishuGrantEncryptor, FeishuGrantEnvelope, FeishuOAuthTransport, FeishuOAuthTransportError,
    FeishuRefreshMaterial, FeishuRefreshMaterialProvider, HttpClient, HttpResponse,
    ReqwestAsyncHttpClient, ReqwestBlockingHttpClient,
};
#[cfg(feature = "postgres")]
pub use postgres::{PostgresFeishuGrantMaterialStore, PostgresFeishuGrantMaterialStoreError};
pub use redaction::SecretString;

use std::fmt;

use oar_core::lark::auth::adapter::LarkAuthRefreshAdapter;
use oar_core::lark::auth::client::{LarkAuthRefreshSafeClient, LarkAuthRefreshSafeClientConfig};

use crate::config::{FeishuOpenApiConfig, FeishuOpenApiConfigError};
use crate::oauth::{
    FeishuOAuthTransport, HttpClientFailure, ReqwestAsyncHttpClient, ReqwestBlockingHttpClient,
};

pub type FeishuAuthRefreshAdapter<P, E, H> =
    LarkAuthRefreshAdapter<LarkAuthRefreshSafeClient<FeishuOAuthTransport<P, E, H>>>;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FeishuAuthRefreshAdapterBuildError {
    InvalidConfig(FeishuOpenApiConfigError),
    HttpClientBuildFailed,
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
    let safe_client_config = LarkAuthRefreshSafeClientConfig {
        max_response_bytes: config.max_response_bytes,
    };
    let transport = FeishuOAuthTransport::new(config, material_provider, encryptor, http_client);
    let safe_client = LarkAuthRefreshSafeClient::with_config(transport, safe_client_config);
    Ok(LarkAuthRefreshAdapter::new(safe_client))
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
mod tests {
    use oar_core::domain::identity::{TenantId, TokenGrantId, TokenGrantState};
    use oar_core::domain::token_refresh::service::AuthRefreshAdapter;
    use oar_core::domain::token_refresh::types::{RefreshOutcome, TokenRefreshGrantSnapshot};
    use oar_core::lark::auth::types::LarkAuthRefreshRequest;
    use serde_json::json;

    use super::*;
    use crate::oauth::{
        FeishuGrantEncryptionInput, FeishuGrantEncryptor, FeishuGrantEnvelope,
        FeishuRefreshMaterial, FeishuRefreshMaterialProvider, HttpClient, HttpRequest,
        HttpResponse,
    };
    use crate::redaction::SecretString;

    const ACCESS_TOKEN: &str = "uat-sensitive-access-token";
    const REFRESH_TOKEN: &str = "urt-sensitive-refresh-token";
    const CLIENT_SECRET: &str = "secret-sensitive-client";

    #[test]
    fn factory_adapter_refreshes_and_returns_encrypted_success() {
        let mut adapter = build_feishu_auth_refresh_adapter(
            FeishuOpenApiConfig::default(),
            FakeMaterialProvider,
            FakeEncryptor,
            FakeHttpClient::from_response(HttpResponse::new(200, success_body())),
        )
        .expect("factory should accept default config");

        match adapter.refresh(&sample_snapshot()) {
            RefreshOutcome::Success {
                rotated_material,
                key_id,
                new_fingerprint,
                ..
            } => {
                assert_eq!(rotated_material.encrypted_primary, vec![11, 12, 13]);
                assert_eq!(rotated_material.encrypted_renewal, vec![21, 22, 23]);
                assert_eq!(key_id, "kms-test");
                assert_eq!(new_fingerprint, "fp-rotated");
            }
            other => panic!("expected success, got {other:?}"),
        }
    }

    #[test]
    fn factory_uses_config_max_response_bytes_for_safe_client_guard() {
        let mut adapter = build_feishu_auth_refresh_adapter(
            FeishuOpenApiConfig {
                max_response_bytes: 8,
                ..FeishuOpenApiConfig::default()
            },
            FakeMaterialProvider,
            FakeEncryptor,
            FakeHttpClient::from_response(HttpResponse::new(200, success_body())),
        )
        .expect("factory should accept max_response_bytes override");

        assert_eq!(adapter.client().config().max_response_bytes, 8);
        match adapter.refresh(&sample_snapshot()) {
            RefreshOutcome::ConfigRequired { safe_error } => {
                assert_eq!(safe_error, "auth_refresh_oversized_response");
                assert!(!safe_error.contains(ACCESS_TOKEN));
                assert!(!safe_error.contains(REFRESH_TOKEN));
            }
            other => panic!("expected config required failure, got {other:?}"),
        }
    }

    #[test]
    fn reqwest_factory_accepts_timeout_config() {
        let adapter = build_reqwest_feishu_auth_refresh_adapter(
            FeishuOpenApiConfig {
                base_url: "https://open.feishu.cn".to_string(),
                max_response_bytes: 1024,
                request_timeout_ms: 1_500,
                connect_timeout_ms: 500,
            },
            FakeMaterialProvider,
            FakeEncryptor,
        )
        .expect("reqwest-backed factory should build");

        let debug = format!("{adapter:?}");
        assert!(!debug.contains(CLIENT_SECRET));
        assert!(!debug.contains(ACCESS_TOKEN));
        assert!(!debug.contains(REFRESH_TOKEN));
    }

    #[test]
    fn async_reqwest_factory_accepts_timeout_config() {
        let adapter = build_async_reqwest_feishu_auth_refresh_adapter(
            FeishuOpenApiConfig {
                base_url: "https://open.feishu.cn".to_string(),
                max_response_bytes: 1024,
                request_timeout_ms: 1_500,
                connect_timeout_ms: 500,
            },
            FakeMaterialProvider,
            FakeEncryptor,
        )
        .expect("async reqwest-backed factory should build");

        let debug = format!("{adapter:?}");
        assert!(!debug.contains(CLIENT_SECRET));
        assert!(!debug.contains(ACCESS_TOKEN));
        assert!(!debug.contains(REFRESH_TOKEN));
    }

    fn sample_snapshot() -> TokenRefreshGrantSnapshot {
        TokenRefreshGrantSnapshot {
            grant_id: TokenGrantId("grant_auth_refresh_1".to_string()),
            tenant_id: TenantId("tenant_auth_refresh_1".to_string()),
            expected_fingerprint: "fp_prev_v1".to_string(),
            state: TokenGrantState::Valid,
            has_refresh_material: true,
            revoked_at: None,
            reauth_required_at: None,
        }
    }

    fn success_body() -> String {
        json!({
            "code": 0,
            "access_token": ACCESS_TOKEN,
            "expires_in": 7200,
            "refresh_token": REFRESH_TOKEN,
            "refresh_token_expires_in": 604800,
            "scope": "offline_access auth:user.id:read",
            "token_type": "Bearer"
        })
        .to_string()
    }

    struct FakeMaterialProvider;

    impl FeishuRefreshMaterialProvider for FakeMaterialProvider {
        type Error = ();

        fn refresh_material(
            &mut self,
            _request: &LarkAuthRefreshRequest,
        ) -> Result<FeishuRefreshMaterial, Self::Error> {
            Ok(FeishuRefreshMaterial {
                client_id: "cli_test".to_string(),
                client_secret: SecretString::new(CLIENT_SECRET),
                refresh_token: SecretString::new(REFRESH_TOKEN),
                scope: Some("offline_access auth:user.id:read".to_string()),
            })
        }
    }

    struct FakeEncryptor;

    impl FeishuGrantEncryptor for FakeEncryptor {
        type Error = ();

        fn encrypt(
            &mut self,
            _input: FeishuGrantEncryptionInput,
        ) -> Result<FeishuGrantEnvelope, Self::Error> {
            Ok(FeishuGrantEnvelope {
                encrypted_primary: vec![11, 12, 13],
                encrypted_renewal: vec![21, 22, 23],
                key_id: "kms-test".to_string(),
                new_fingerprint: "fp-rotated".to_string(),
                refreshed_at_ms: 1_779_465_600_000,
                expires_at_ms: Some(1_779_472_800_000),
            })
        }
    }

    #[derive(Clone)]
    struct FakeHttpClient {
        response: HttpResponse,
    }

    impl FakeHttpClient {
        fn from_response(response: HttpResponse) -> Self {
            Self { response }
        }
    }

    impl HttpClient for FakeHttpClient {
        fn post_json(&mut self, _request: HttpRequest) -> Result<HttpResponse, HttpClientFailure> {
            Ok(self.response.clone())
        }
    }
}

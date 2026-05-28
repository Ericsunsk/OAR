use oar_core::domain::identity::{TenantId, TokenGrantId, TokenGrantState};
#[cfg(feature = "postgres")]
use oar_core::domain::token_refresh::service::AsyncAuthRefreshAdapter;
use oar_core::domain::token_refresh::service::AuthRefreshAdapter;
use oar_core::domain::token_refresh::types::{RefreshOutcome, TokenRefreshGrantSnapshot};
use oar_core::lark::auth::types::FeishuAuthRefreshRequest;
use serde_json::json;

use super::*;
#[cfg(feature = "postgres")]
use crate::oauth::AsyncHttpClient;
use crate::oauth::{
    FeishuGrantEncryptionInput, FeishuGrantEncryptor, FeishuGrantEnvelope, FeishuRefreshMaterial,
    FeishuRefreshMaterialProvider, HttpClient, HttpClientFailure, HttpRequest, HttpResponse,
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

#[test]
fn static_key_resolver_returns_key_only_for_matching_key_id() {
    let key = [7; 32];
    let mut resolver = StaticAesGcmKeyResolver::new("key-v1", key);

    assert_eq!(resolver.key_for("key-v1"), Ok(key));
    assert_eq!(
        resolver.key_for("key-v2"),
        Err(StaticAesGcmKeyResolverError)
    );

    let debug = format!("{resolver:?}");
    assert!(!debug.contains("key-v1"));
    assert!(!debug.contains("7, 7"));
}

#[test]
fn postgres_refresh_env_config_parses_required_runtime_secrets() {
    let config = PostgresFeishuAuthRefreshEnvConfig::from_env_map(&|key| match key {
        "OAR_FEISHU_APP_ID" => Some("cli_prod".to_string()),
        "OAR_FEISHU_APP_SECRET" => Some("very-secret-value".to_string()),
        "OAR_GRANT_KEY_ID" => Some("key-prod-v1".to_string()),
        "OAR_GRANT_KEY_HEX" => Some("11".repeat(32)),
        _ => None,
    })
    .expect("env config should parse");

    assert_eq!(config.app_id, "cli_prod");
    assert_eq!(config.grant_key_id, "key-prod-v1");
    assert_eq!(config.grant_key_material, [0x11; 32]);
    assert!(!format!("{config:?}").contains("very-secret-value"));
    assert!(!format!("{config:?}").contains("key-prod-v1"));
}

#[test]
fn postgres_refresh_env_config_rejects_missing_or_empty_required_values() {
    let missing_secret = PostgresFeishuAuthRefreshEnvConfig::from_env_map(&|key| match key {
        "OAR_FEISHU_APP_ID" => Some("cli_prod".to_string()),
        "OAR_GRANT_KEY_ID" => Some("key-prod-v1".to_string()),
        "OAR_GRANT_KEY_HEX" => Some("11".repeat(32)),
        _ => None,
    })
    .expect_err("missing app secret should fail");
    assert_eq!(
        missing_secret,
        PostgresFeishuAuthRefreshEnvConfigError::MissingAppSecret
    );

    let empty_key_id = PostgresFeishuAuthRefreshEnvConfig::from_env_map(&|key| match key {
        "OAR_FEISHU_APP_ID" => Some("cli_prod".to_string()),
        "OAR_FEISHU_APP_SECRET" => Some("very-secret-value".to_string()),
        "OAR_GRANT_KEY_ID" => Some("   ".to_string()),
        "OAR_GRANT_KEY_HEX" => Some("11".repeat(32)),
        _ => None,
    })
    .expect_err("empty grant key id should fail");
    assert_eq!(
        empty_key_id,
        PostgresFeishuAuthRefreshEnvConfigError::MissingGrantKeyId
    );
}

#[test]
fn postgres_refresh_env_config_rejects_invalid_grant_key_hex_without_leaking_input() {
    let bad_format_value = "not-hex-sensitive-key";
    let bad_format = PostgresFeishuAuthRefreshEnvConfig::from_env_map(&|key| match key {
        "OAR_FEISHU_APP_ID" => Some("cli_prod".to_string()),
        "OAR_FEISHU_APP_SECRET" => Some("super-secret-app-secret".to_string()),
        "OAR_GRANT_KEY_ID" => Some("key-prod-v1".to_string()),
        "OAR_GRANT_KEY_HEX" => Some(bad_format_value.to_string()),
        _ => None,
    })
    .expect_err("invalid hex must fail");
    assert_eq!(
        bad_format,
        PostgresFeishuAuthRefreshEnvConfigError::InvalidGrantKeyHex
    );
    let rendered_bad_format = bad_format.to_string();
    assert!(!rendered_bad_format.contains(bad_format_value));
    assert!(!rendered_bad_format.contains("super-secret-app-secret"));

    let bad_length_value = "22".repeat(31);
    let bad_length = PostgresFeishuAuthRefreshEnvConfig::from_env_map(&|key| match key {
        "OAR_FEISHU_APP_ID" => Some("cli_prod".to_string()),
        "OAR_FEISHU_APP_SECRET" => Some("super-secret-app-secret".to_string()),
        "OAR_GRANT_KEY_ID" => Some("key-prod-v1".to_string()),
        "OAR_GRANT_KEY_HEX" => Some(bad_length_value.clone()),
        _ => None,
    })
    .expect_err("invalid key length must fail");
    assert_eq!(
        bad_length,
        PostgresFeishuAuthRefreshEnvConfigError::InvalidGrantKeyHex
    );
    let rendered_bad_length = format!("{bad_length:?} {}", bad_length);
    assert!(!rendered_bad_length.contains(&bad_length_value));
    assert!(!rendered_bad_length.contains("super-secret-app-secret"));
}

#[cfg(feature = "postgres")]
#[test]
fn postgres_async_factory_builds_send_async_adapter_without_secret_debug() {
    fn assert_async_adapter<T: AsyncAuthRefreshAdapter + Send>(_value: &T) {}

    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("test tokio runtime should build");
    let _runtime_guard = runtime.enter();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://oar:oar@127.0.0.1:5432/oar_test")
        .expect("lazy postgres pool should not connect");
    let adapter = build_postgres_async_feishu_auth_refresh_adapter(
        pool,
        FeishuOpenApiConfig {
            base_url: "https://open.feishu.cn".to_string(),
            max_response_bytes: 1024,
            request_timeout_ms: 1_500,
            connect_timeout_ms: 500,
        },
        "cli_test",
        SecretString::new(CLIENT_SECRET),
        "key-prod-v1",
        [9; 32],
    )
    .expect("postgres async production factory should build");

    assert_async_adapter(&adapter);
    let debug = format!("{adapter:?}");
    assert!(!debug.contains(CLIENT_SECRET));
    assert!(!debug.contains("key-prod-v1"));
    assert!(!debug.contains("9, 9"));
    assert!(!debug.contains(ACCESS_TOKEN));
    assert!(!debug.contains(REFRESH_TOKEN));
}

#[cfg(feature = "postgres")]
#[test]
fn postgres_factory_with_injected_http_builds_adapter_without_network() {
    fn assert_async_adapter<T: AsyncAuthRefreshAdapter + Send>(_value: &T) {}

    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("test tokio runtime should build");
    let _runtime_guard = runtime.enter();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://oar:oar@127.0.0.1:5432/oar_test")
        .expect("lazy postgres pool should not connect");
    let adapter = build_postgres_feishu_auth_refresh_adapter_with_http(
        pool,
        FeishuOpenApiConfig {
            base_url: "https://open.feishu.cn".to_string(),
            max_response_bytes: 1024,
            request_timeout_ms: 1_500,
            connect_timeout_ms: 500,
        },
        "cli_test",
        SecretString::new(CLIENT_SECRET),
        "key-prod-v1",
        [9; 32],
        FakeAsyncHttpClient,
    )
    .expect("postgres injectable factory should build");

    assert_async_adapter(&adapter);
    let debug = format!("{adapter:?}");
    assert!(!debug.contains(CLIENT_SECRET));
    assert!(!debug.contains("key-prod-v1"));
    assert!(!debug.contains("9, 9"));
    assert!(!debug.contains(ACCESS_TOKEN));
    assert!(!debug.contains(REFRESH_TOKEN));
}

#[cfg(feature = "postgres")]
#[test]
fn postgres_async_factory_rejects_invalid_inputs_without_secret_debug() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("test tokio runtime should build");
    let _runtime_guard = runtime.enter();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://oar:oar@127.0.0.1:5432/oar_test")
        .expect("lazy postgres pool should not connect");

    let empty_client_id = build_postgres_feishu_auth_refresh_adapter_with_http(
        pool.clone(),
        FeishuOpenApiConfig::default(),
        " ",
        SecretString::new(CLIENT_SECRET),
        "key-prod-v1",
        [9; 32],
        FakeAsyncHttpClient,
    )
    .expect_err("empty client id should be rejected");
    assert_eq!(
        empty_client_id,
        FeishuAuthRefreshAdapterBuildError::EmptyClientId
    );
    assert!(!format!("{empty_client_id:?}").contains(CLIENT_SECRET));
    assert!(!empty_client_id.to_string().contains(CLIENT_SECRET));

    let empty_key_id = build_postgres_feishu_auth_refresh_adapter_with_http(
        pool,
        FeishuOpenApiConfig::default(),
        "cli_test",
        SecretString::new(CLIENT_SECRET),
        " ",
        [9; 32],
        FakeAsyncHttpClient,
    )
    .expect_err("empty grant key id should be rejected");
    assert_eq!(
        empty_key_id,
        FeishuAuthRefreshAdapterBuildError::EmptyGrantKeyId
    );
    assert!(!format!("{empty_key_id:?}").contains(CLIENT_SECRET));
    assert!(!empty_key_id.to_string().contains(CLIENT_SECRET));
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
        _request: &FeishuAuthRefreshRequest,
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

#[cfg(feature = "postgres")]
struct FakeAsyncHttpClient;

#[cfg(feature = "postgres")]
#[async_trait::async_trait]
impl AsyncHttpClient for FakeAsyncHttpClient {
    async fn post_json(
        &mut self,
        _request: HttpRequest,
    ) -> Result<HttpResponse, HttpClientFailure> {
        Ok(HttpResponse::new(200, success_body()))
    }
}

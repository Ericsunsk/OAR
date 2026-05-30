use super::support::{
    sample_snapshot, success_body, FakeEncryptor, FakeHttpClient, FakeMaterialProvider,
    ACCESS_TOKEN, CLIENT_SECRET, REFRESH_TOKEN,
};
use super::*;

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

use oar_core::domain::identity::{TenantId, TokenGrantId, TokenGrantState};
use oar_core::domain::token_refresh::types::TokenRefreshGrantSnapshot;
use oar_core::lark::auth::types::{LarkAuthGrantState, LarkAuthRefreshRequest};

use crate::material::compose_encrypted_grant_blob;
use crate::oauth::{
    FeishuGrantEncryptionInput, FeishuGrantEnvelope, FeishuRefreshMaterial, HttpClientFailure,
    HttpResponse,
};
use crate::redaction::SecretString;
use crate::FeishuOpenApiConfig;

use super::fakes::{FakeEncryptor, FakeHttpClient, FakeMaterialProvider, FixedClock};
use crate::crypto::AesGcmGrantEncryptor;
use crate::material::StoredFeishuGrantMaterial;
use crate::FeishuGrantEncryptor;

pub(crate) const ACCESS_TOKEN: &str = "uat-sensitive-access-token";
pub(crate) const REFRESH_TOKEN: &str = "urt-sensitive-refresh-token";
pub(crate) const CLIENT_SECRET: &str = "secret-sensitive-client";

pub(crate) fn sample_transport(
    response: HttpResponse,
) -> crate::oauth::FeishuOAuthTransport<FakeMaterialProvider, FakeEncryptor, FakeHttpClient> {
    crate::oauth::FeishuOAuthTransport::new(
        FeishuOpenApiConfig::default(),
        FakeMaterialProvider,
        FakeEncryptor,
        FakeHttpClient::from_response(response),
    )
}

pub(crate) fn transport_with_http_error(
    failure: HttpClientFailure,
) -> crate::oauth::FeishuOAuthTransport<FakeMaterialProvider, FakeEncryptor, FakeHttpClient> {
    crate::oauth::FeishuOAuthTransport::new(
        FeishuOpenApiConfig::default(),
        FakeMaterialProvider,
        FakeEncryptor,
        FakeHttpClient::from_error(failure),
    )
}

pub(crate) fn sample_request() -> LarkAuthRefreshRequest {
    LarkAuthRefreshRequest {
        grant_id: "grant-1".to_string(),
        tenant_id: "tenant-1".to_string(),
        expected_fingerprint: "fp-prev".to_string(),
        grant_state: LarkAuthGrantState::NeedsRefresh,
        has_refresh_material: true,
        is_revoked: false,
        reauth_marked: false,
    }
}

pub(crate) fn sample_material() -> FeishuRefreshMaterial {
    FeishuRefreshMaterial {
        client_id: "cli_test".to_string(),
        client_secret: SecretString::new(CLIENT_SECRET),
        refresh_token: SecretString::new(REFRESH_TOKEN),
        scope: Some("offline_access auth:user.id:read".to_string()),
    }
}

pub(crate) fn sample_envelope() -> FeishuGrantEnvelope {
    FeishuGrantEnvelope {
        encrypted_primary: vec![11, 12, 13],
        encrypted_renewal: vec![21, 22, 23],
        key_id: "kms-test".to_string(),
        new_fingerprint: "fp-rotated".to_string(),
        refreshed_at_ms: 1_779_465_600_000,
        expires_at_ms: Some(1_779_472_800_000),
    }
}

pub(crate) fn success_body() -> String {
    serde_json::json!({
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

pub(crate) fn error_body(code: i64) -> String {
    serde_json::json!({
        "code": code,
        "error": "server_error",
        "error_description": "redacted in adapter output"
    })
    .to_string()
}

pub(crate) fn assert_no_secret(rendered: &str) {
    assert!(!rendered.contains(ACCESS_TOKEN));
    assert!(!rendered.contains(REFRESH_TOKEN));
    assert!(!rendered.contains(CLIENT_SECRET));
    assert!(!rendered.contains("Bearer"));
    assert!(!rendered.contains("secret-sensitive"));
}

pub(crate) fn stored_material_from_plaintext(
    key: [u8; 32],
    refresh_token: &str,
) -> StoredFeishuGrantMaterial {
    let mut encryptor = AesGcmGrantEncryptor::with_clock(
        "key-1",
        key,
        FixedClock {
            now_ms: 1_779_465_000_000,
        },
    );
    let envelope = encryptor
        .encrypt(FeishuGrantEncryptionInput {
            grant_id: "grant-1".to_string(),
            tenant_id: "tenant-1".to_string(),
            expected_fingerprint: "fp-prev".to_string(),
            access_token: SecretString::new("uat-stored-primary"),
            refresh_token: SecretString::new(refresh_token),
            expires_in_seconds: 7200,
            refresh_token_expires_in_seconds: Some(604800),
            token_type: Some("Bearer".to_string()),
            scope: Some("offline_access auth:user.id:read".to_string()),
        })
        .expect("seed encryption should succeed");

    StoredFeishuGrantMaterial {
        grant_id: "grant-1".to_string(),
        tenant_id: "tenant-1".to_string(),
        encrypted_oauth_grant: compose_encrypted_grant_blob(
            envelope.encrypted_primary,
            envelope.encrypted_renewal,
        ),
        oauth_grant_key_id: envelope.key_id,
        oauth_grant_fingerprint: "fp-current".to_string(),
        scope: Some("offline_access auth:user.id:read".to_string()),
    }
}

pub(crate) fn snapshot() -> TokenRefreshGrantSnapshot {
    TokenRefreshGrantSnapshot {
        grant_id: TokenGrantId("grant-1".to_string()),
        tenant_id: TenantId("tenant-1".to_string()),
        expected_fingerprint: "fp-current".to_string(),
        state: TokenGrantState::NeedsRefresh,
        has_refresh_material: true,
        revoked_at: None,
        reauth_required_at: None,
    }
}

pub(crate) fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

pub(crate) fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime should build")
}

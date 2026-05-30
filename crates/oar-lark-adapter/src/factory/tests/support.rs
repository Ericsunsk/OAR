use oar_core::domain::identity::{TenantId, TokenGrantId, TokenGrantState};
use oar_core::domain::token_refresh::types::TokenRefreshGrantSnapshot;
use oar_core::lark::auth::types::FeishuAuthRefreshRequest;
use serde_json::json;

#[cfg(feature = "postgres")]
use crate::oauth::AsyncHttpClient;
use crate::oauth::{
    FeishuGrantEncryptionInput, FeishuGrantEncryptor, FeishuGrantEnvelope, FeishuRefreshMaterial,
    FeishuRefreshMaterialProvider, HttpClient, HttpClientFailure, HttpRequest, HttpResponse,
};
use crate::redaction::SecretString;

pub(super) const ACCESS_TOKEN: &str = "uat-sensitive-access-token";
pub(super) const REFRESH_TOKEN: &str = "urt-sensitive-refresh-token";
pub(super) const CLIENT_SECRET: &str = "secret-sensitive-client";

pub(super) fn sample_snapshot() -> TokenRefreshGrantSnapshot {
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

pub(super) fn success_body() -> String {
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

pub(super) struct FakeMaterialProvider;

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

pub(super) struct FakeEncryptor;

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
pub(super) struct FakeHttpClient {
    response: HttpResponse,
}

impl FakeHttpClient {
    pub(super) fn from_response(response: HttpResponse) -> Self {
        Self { response }
    }
}

impl HttpClient for FakeHttpClient {
    fn post_json(&mut self, _request: HttpRequest) -> Result<HttpResponse, HttpClientFailure> {
        Ok(self.response.clone())
    }
}

#[cfg(feature = "postgres")]
pub(super) struct FakeAsyncHttpClient;

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

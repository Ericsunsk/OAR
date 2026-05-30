use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use oar_core::action::audit_event::{AuditActor, AuditActorKind};
use oar_core::action::token_refresh_audit::TokenRefreshAuditContext;
use oar_core::domain::identity::{ActorKind, ScopeBoundary, TokenGrantState};
use oar_core::storage::postgres::{EncryptedTokenGrantRecord, PostgresTokenGrantRepository};
use oar_lark_adapter::oauth::{HttpClientFailure, HttpRequest};
use oar_lark_adapter::{
    AesGcmGrantEncryptor, AesGcmKeyResolver, AsyncHttpClient, FeishuGrantEncryptionInput,
    FeishuGrantEncryptor, GrantTimeSource, HttpResponse, PostgresFeishuGrantMaterialStore,
    SecretString, StaticFeishuAppCredentialProvider,
};
use sqlx::PgPool;

use super::constants::{
    ACTOR_ID, CLIENT_SECRET, IDENTITY_ID, KEY_ID, NEW_ACCESS_TOKEN, NEW_REFRESH_TOKEN, OLD_FP,
    SEED_ACCESS_TOKEN, SEED_REFRESH_TOKEN, TENANT_ID,
};

#[derive(Clone)]
pub(crate) struct RecordingAsyncHttpClient {
    result: Result<HttpResponse, HttpClientFailure>,
    requests: Arc<Mutex<Vec<HttpRequest>>>,
}

impl RecordingAsyncHttpClient {
    pub(crate) fn from_response(response: HttpResponse) -> Self {
        Self::from_result(Ok(response))
    }

    pub(crate) fn from_result(result: Result<HttpResponse, HttpClientFailure>) -> Self {
        Self {
            result,
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn requests(&self) -> Vec<HttpRequest> {
        self.requests
            .lock()
            .expect("fake http request mutex")
            .clone()
    }
}

#[async_trait]
impl AsyncHttpClient for RecordingAsyncHttpClient {
    async fn post_json(&mut self, request: HttpRequest) -> Result<HttpResponse, HttpClientFailure> {
        self.requests
            .lock()
            .expect("fake http request mutex")
            .push(request);
        self.result.clone()
    }
}

#[derive(Clone)]
pub(crate) struct FixedKeyResolver {
    pub(crate) key: [u8; 32],
}

impl AesGcmKeyResolver for FixedKeyResolver {
    type Error = std::convert::Infallible;

    fn key_for(&mut self, key_id: &str) -> Result<[u8; 32], Self::Error> {
        assert_eq!(key_id, KEY_ID);
        Ok(self.key)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct FixedClock {
    pub(crate) now_ms: u64,
}

impl GrantTimeSource for FixedClock {
    fn now_ms(&self) -> u64 {
        self.now_ms
    }
}

pub(crate) fn success_body() -> String {
    serde_json::json!({
        "code": 0,
        "access_token": NEW_ACCESS_TOKEN,
        "expires_in": 7200,
        "refresh_token": NEW_REFRESH_TOKEN,
        "refresh_token_expires_in": 604800,
        "scope": "offline_access auth:user.id:read okr.progress.write",
        "token_type": "Bearer"
    })
    .to_string()
}

pub(crate) fn failure_body(code: i64) -> String {
    serde_json::json!({
        "code": code,
        "error": "server_error",
        "error_description": "redacted"
    })
    .to_string()
}

pub(crate) fn assert_feishu_refresh_headers(headers: &[(String, String)]) {
    assert_eq!(
        headers,
        &[
            (
                "Content-Type".to_string(),
                "application/json; charset=utf-8".to_string()
            ),
            ("Accept".to_string(), "application/json".to_string()),
            (
                "User-Agent".to_string(),
                format!("oar-lark-adapter/{}", env!("CARGO_PKG_VERSION"))
            )
        ]
    );
}

pub(crate) fn assert_no_sensitive_text(text: &str) {
    for needle in [
        SEED_ACCESS_TOKEN,
        SEED_REFRESH_TOKEN,
        NEW_ACCESS_TOKEN,
        NEW_REFRESH_TOKEN,
        CLIENT_SECRET,
        "access_token",
        "refresh_token",
        "authorization_code",
        "Authorization",
        "Bearer",
        "encrypted_primary",
        "encrypted_renewal",
        OLD_FP,
        "fp-current",
    ] {
        assert!(
            !text.contains(needle),
            "sensitive marker leaked into text: {needle}"
        );
    }
}

pub(crate) fn assert_no_byte_secret(bytes: &[u8]) {
    for needle in [
        SEED_ACCESS_TOKEN,
        SEED_REFRESH_TOKEN,
        NEW_ACCESS_TOKEN,
        NEW_REFRESH_TOKEN,
    ] {
        assert!(
            !contains_subslice(bytes, needle.as_bytes()),
            "sensitive marker leaked into encrypted blob: {needle}"
        );
    }
}

pub(crate) fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && needle.len() <= haystack.len()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

pub(crate) fn encrypted_blob_from_plaintext(
    key: [u8; 32],
    now_ms: u64,
    access_token: &str,
    refresh_token: &str,
) -> Vec<u8> {
    let mut encryptor = AesGcmGrantEncryptor::with_clock(KEY_ID, key, FixedClock { now_ms });
    let envelope = FeishuGrantEncryptor::encrypt(
        &mut encryptor,
        FeishuGrantEncryptionInput {
            grant_id: super::constants::GRANT_ID.to_string(),
            tenant_id: TENANT_ID.to_string(),
            expected_fingerprint: "seed-fingerprint".to_string(),
            access_token: SecretString::new(access_token),
            refresh_token: SecretString::new(refresh_token),
            expires_in_seconds: 60,
            refresh_token_expires_in_seconds: Some(120),
            token_type: Some("Bearer".to_string()),
            scope: Some("offline_access auth:user.id:read okr.progress.write".to_string()),
        },
    )
    .expect("seed grant encryption should succeed");

    oar_lark_adapter::material::compose_encrypted_grant_blob(
        envelope.encrypted_primary,
        envelope.encrypted_renewal,
    )
}

pub(crate) fn make_material_provider(
    pool: PgPool,
    key: [u8; 32],
) -> oar_lark_adapter::FeishuStoredRefreshMaterialProvider<
    PostgresFeishuGrantMaterialStore,
    FixedKeyResolver,
    StaticFeishuAppCredentialProvider,
> {
    oar_lark_adapter::FeishuStoredRefreshMaterialProvider::new(
        PostgresFeishuGrantMaterialStore::new(pool),
        FixedKeyResolver { key },
        StaticFeishuAppCredentialProvider::new("cli_test", SecretString::new(CLIENT_SECRET)),
    )
}

pub(crate) async fn seed_refresh_candidate_grant(
    pool: &PgPool,
    grant_id: &str,
    blob: Vec<u8>,
) -> Result<(), oar_core::storage::postgres::PostgresRepositoryError> {
    PostgresTokenGrantRepository::new(pool.clone())
        .upsert_encrypted_grant(&EncryptedTokenGrantRecord {
            id: grant_id.to_string(),
            tenant_id: TENANT_ID.to_string(),
            identity_id: IDENTITY_ID.to_string(),
            actor_kind: ActorKind::User,
            scope_boundary: ScopeBoundary::User,
            scopes: vec![
                "offline_access".to_string(),
                "auth:user.id:read".to_string(),
                "okr.progress.write".to_string(),
            ],
            state: TokenGrantState::NeedsRefresh,
            issued_at_ms: 1_779_460_000_000,
            expires_at_ms: Some(1_779_465_500_000),
            refreshed_at_ms: Some(1_779_465_000_000),
            revoked_at_ms: None,
            reauth_required_at_ms: None,
            last_refresh_error: Some("old-error".to_string()),
            encrypted_oauth_grant: blob,
            oauth_grant_key_id: KEY_ID.to_string(),
            oauth_grant_fingerprint: OLD_FP.to_string(),
            revocation_reason: None,
        })
        .await?;
    Ok(())
}

pub(crate) fn audit_context(trace_id: &str, sequence: u64) -> TokenRefreshAuditContext {
    TokenRefreshAuditContext {
        trace_id: trace_id.to_string(),
        sequence,
        occurred_at_ms: 1_779_466_000_123,
        actor: AuditActor {
            kind: AuditActorKind::User,
            actor_id: ACTOR_ID.to_string(),
            display_name: Some("Reviewer".to_string()),
        },
        workspace_id: None,
    }
}

pub(crate) async fn seed_identity_graph(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO tenants (id, display_name, status)
        VALUES ($1, $2, 'active')
        "#,
    )
    .bind(TENANT_ID)
    .bind("Adapter integration tenant")
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO workspace_users (id, tenant_id, display_name, status)
        VALUES ($1, $2, $3, 'active')
        "#,
    )
    .bind(super::constants::USER_ID)
    .bind(TENANT_ID)
    .bind("Adapter integration user")
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO lark_identities (id, tenant_id, actor_kind, actor_external_id, display_name)
        VALUES ($1, $2, 'user', $3, $4)
        "#,
    )
    .bind(IDENTITY_ID)
    .bind(TENANT_ID)
    .bind("ext_adapter_pg_refresh")
    .bind("Adapter integration identity")
    .execute(pool)
    .await?;

    Ok(())
}

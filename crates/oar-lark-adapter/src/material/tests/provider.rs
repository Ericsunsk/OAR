use std::fmt;

use oar_core::lark::auth::types::{FeishuAuthRefreshRequest, LarkAuthGrantState};

use crate::crypto::{AesGcmGrantDecryptError, AesGcmGrantEncryptor, AesGcmGrantEncryptorError};
use crate::material::blob::{compose_encrypted_grant_blob, parse_encrypted_grant_blob};
use crate::material::{
    AesGcmKeyResolver, AesGcmRefreshMaterialProvider, AesGcmRefreshMaterialProviderError,
    FeishuGrantMaterialStore, FeishuStoredRefreshMaterialProvider, StoredFeishuGrantMaterial,
};
use crate::oauth::{
    FeishuGrantEncryptionInput, FeishuGrantEncryptor, FeishuRefreshMaterialProvider,
};
use crate::redaction::SecretString;

#[derive(Clone, Debug)]
struct FakeStore {
    stored: StoredFeishuGrantMaterial,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FakeStoreError;
impl fmt::Display for FakeStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("fake store err")
    }
}
impl std::error::Error for FakeStoreError {}

impl FeishuGrantMaterialStore for FakeStore {
    type Error = FakeStoreError;
    fn load(
        &mut self,
        _request: &FeishuAuthRefreshRequest,
    ) -> Result<StoredFeishuGrantMaterial, Self::Error> {
        Ok(self.stored.clone())
    }
}

#[derive(Clone)]
struct FakeResolver {
    key: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FakeResolverError;
impl fmt::Display for FakeResolverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("fake resolver err")
    }
}
impl std::error::Error for FakeResolverError {}

impl AesGcmKeyResolver for FakeResolver {
    type Error = FakeResolverError;
    fn key_for(&mut self, _key_id: &str) -> Result<[u8; 32], Self::Error> {
        Ok(self.key)
    }
}

#[test]
fn decrypts_renewal_token_from_blob() {
    let key = [7; 32];
    let request = sample_request();
    let stored = sample_stored_material(key, "refresh-secret-1", "access-secret-1");
    let mut provider =
        AesGcmRefreshMaterialProvider::new(FakeStore { stored }, FakeResolver { key });

    let material = provider
        .decrypted_grant_material(&request)
        .expect("refresh material should load");

    assert_eq!(material.scope, Some("offline_access".to_string()));
    assert_eq!(material.refresh_token.expose_secret(), "refresh-secret-1");
}

#[test]
fn fails_closed_on_grant_tenant_fingerprint_mismatch_without_leaks() {
    let key = [3; 32];
    let mut bad_grant_req = sample_request();
    bad_grant_req.grant_id = "grant-other".to_string();
    let stored = sample_stored_material(key, "refresh-secret-2", "access-secret-2");
    let mut provider = AesGcmRefreshMaterialProvider::new(
        FakeStore {
            stored: stored.clone(),
        },
        FakeResolver { key },
    );
    let err = provider
        .decrypted_grant_material(&bad_grant_req)
        .expect_err("grant mismatch should fail");
    assert!(matches!(
        err,
        AesGcmRefreshMaterialProviderError::GrantMismatch
    ));

    let mut bad_tenant_req = sample_request();
    bad_tenant_req.tenant_id = "tenant-other".to_string();
    let mut provider = AesGcmRefreshMaterialProvider::new(
        FakeStore {
            stored: stored.clone(),
        },
        FakeResolver { key },
    );
    let err = provider
        .decrypted_grant_material(&bad_tenant_req)
        .expect_err("tenant mismatch should fail");
    assert!(matches!(
        err,
        AesGcmRefreshMaterialProviderError::GrantMismatch
    ));

    let mut bad_fp_req = sample_request();
    bad_fp_req.expected_fingerprint = "fp-other".to_string();
    let mut provider =
        AesGcmRefreshMaterialProvider::new(FakeStore { stored }, FakeResolver { key });
    let err = provider
        .decrypted_grant_material(&bad_fp_req)
        .expect_err("fingerprint mismatch should fail");
    assert!(matches!(
        err,
        AesGcmRefreshMaterialProviderError::FingerprintMismatch
    ));

    for rendered in [format!("{err:?}"), err.to_string()] {
        assert!(!rendered.contains("fp-other"));
        assert!(!rendered.contains("fp-current"));
        assert!(!rendered.contains("refresh-secret-2"));
        assert!(!rendered.contains("access-secret-2"));
    }
}

#[test]
fn malformed_blob_or_envelope_or_wrong_key_fail_closed_without_leaks() {
    let key = [2; 32];
    let request = sample_request();

    let mut stored = sample_stored_material(key, "refresh-secret-3", "access-secret-3");
    stored.encrypted_oauth_grant = vec![0, 1, 2];
    let mut provider =
        AesGcmRefreshMaterialProvider::new(FakeStore { stored }, FakeResolver { key });
    let err = provider
        .decrypted_grant_material(&request)
        .expect_err("malformed blob should fail");
    assert!(matches!(
        err,
        AesGcmRefreshMaterialProviderError::MalformedGrantMaterial
    ));

    let mut stored = sample_stored_material(key, "refresh-secret-3", "access-secret-3");
    let (_, renewal) = parse_encrypted_grant_blob(&stored.encrypted_oauth_grant).expect("blob");
    let mut broken_renewal = renewal.to_vec();
    broken_renewal[0] = 9;
    stored.encrypted_oauth_grant = compose_encrypted_grant_blob(vec![1, 2, 3], broken_renewal);
    let mut provider =
        AesGcmRefreshMaterialProvider::new(FakeStore { stored }, FakeResolver { key });
    let err = provider
        .decrypted_grant_material(&request)
        .expect_err("invalid envelope should fail");
    assert!(matches!(
        err,
        AesGcmRefreshMaterialProviderError::DecryptFailed
    ));

    let stored = sample_stored_material(key, "refresh-secret-3", "access-secret-3");
    let mut provider =
        AesGcmRefreshMaterialProvider::new(FakeStore { stored }, FakeResolver { key: [8; 32] });
    let err = provider
        .decrypted_grant_material(&request)
        .expect_err("wrong key should fail");
    assert!(matches!(
        err,
        AesGcmRefreshMaterialProviderError::DecryptFailed
    ));

    for rendered in [format!("{err:?}"), err.to_string()] {
        assert!(!rendered.contains("refresh-secret-3"));
        assert!(!rendered.contains("access-secret-3"));
        assert!(!rendered.contains("fp-current"));
        assert!(!rendered.contains(&hex::encode([8; 32])));
    }
}

#[test]
fn does_not_decrypt_primary_token() {
    let key = [6; 32];
    let request = sample_request();
    let stored = sample_stored_material(key, "refresh-secret-4", "access-secret-4");
    let (_, renewal) = parse_encrypted_grant_blob(&stored.encrypted_oauth_grant).expect("blob");
    let bad_primary = vec![1, 2, 3, 4, 5];
    let stored = StoredFeishuGrantMaterial {
        encrypted_oauth_grant: compose_encrypted_grant_blob(bad_primary, renewal.to_vec()),
        ..stored
    };
    let mut provider =
        AesGcmRefreshMaterialProvider::new(FakeStore { stored }, FakeResolver { key });

    let material = provider
        .decrypted_grant_material(&request)
        .expect("primary should not be decrypted");
    assert_eq!(material.refresh_token.expose_secret(), "refresh-secret-4");
}

#[test]
fn stored_provider_combines_decrypted_renewal_with_app_credentials() {
    let key = [4; 32];
    let request = sample_request();
    let stored = sample_stored_material(key, "refresh-secret-5", "access-secret-5");
    let credential_provider = crate::credentials::StaticFeishuAppCredentialProvider::new(
        "client-1",
        SecretString::new("client-secret-1"),
    );
    let mut provider = FeishuStoredRefreshMaterialProvider::new(
        FakeStore { stored },
        FakeResolver { key },
        credential_provider,
    );

    let material = provider
        .refresh_material(&request)
        .expect("stored provider should compose material");

    assert_eq!(material.client_id, "client-1");
    assert_eq!(material.client_secret.expose_secret(), "client-secret-1");
    assert_eq!(material.refresh_token.expose_secret(), "refresh-secret-5");
    assert_eq!(material.scope, Some("offline_access".to_string()));

    for rendered in [format!("{provider:?}"), provider.to_string()] {
        assert!(!rendered.contains("client-secret-1"));
        assert!(!rendered.contains("refresh-secret-5"));
        assert!(!rendered.contains("access-secret-5"));
        assert!(!rendered.contains("fp-current"));
    }
}

#[test]
fn decrypt_helper_error_mapping_is_non_sensitive() {
    let err = AesGcmGrantDecryptError::InvalidEnvelope;
    let rendered = format!("{err}");
    assert!(!rendered.contains("token"));
    assert!(!rendered.contains("fingerprint"));
}

fn sample_request() -> FeishuAuthRefreshRequest {
    FeishuAuthRefreshRequest {
        grant_id: "grant-1".to_string(),
        tenant_id: "tenant-1".to_string(),
        expected_fingerprint: "fp-current".to_string(),
        grant_state: LarkAuthGrantState::NeedsRefresh,
        has_refresh_material: true,
        is_revoked: false,
        reauth_marked: false,
    }
}

fn sample_stored_material(
    key: [u8; 32],
    refresh_token: &str,
    access_token: &str,
) -> StoredFeishuGrantMaterial {
    let mut encryptor = AesGcmGrantEncryptor::new("key-1", key);
    let envelope = encryptor
        .encrypt(FeishuGrantEncryptionInput {
            grant_id: "grant-1".to_string(),
            tenant_id: "tenant-1".to_string(),
            expected_fingerprint: "fp-prev".to_string(),
            access_token: SecretString::new(access_token),
            refresh_token: SecretString::new(refresh_token),
            expires_in_seconds: 60,
            refresh_token_expires_in_seconds: Some(120),
            token_type: Some("Bearer".to_string()),
            scope: Some("offline_access".to_string()),
        })
        .unwrap_or_else(|err: AesGcmGrantEncryptorError| panic!("encrypt failed: {err}"));
    StoredFeishuGrantMaterial {
        grant_id: "grant-1".to_string(),
        tenant_id: "tenant-1".to_string(),
        encrypted_oauth_grant: compose_encrypted_grant_blob(
            envelope.encrypted_primary,
            envelope.encrypted_renewal,
        ),
        oauth_grant_key_id: envelope.key_id,
        oauth_grant_fingerprint: "fp-current".to_string(),
        scope: Some("offline_access".to_string()),
    }
}

use std::fmt;

use oar_core::lark::auth::types::{FeishuAuthRefreshRequest, LarkAuthGrantState};

use crate::crypto::{AesGcmGrantEncryptor, AesGcmGrantEncryptorError};
use crate::material::blob::compose_encrypted_grant_blob;
use crate::material::{AesGcmKeyResolver, FeishuGrantMaterialStore, StoredFeishuGrantMaterial};
use crate::oauth::{FeishuGrantEncryptionInput, FeishuGrantEncryptor};
use crate::redaction::SecretString;

#[derive(Clone, Debug)]
pub(super) struct FakeStore {
    pub(super) stored: StoredFeishuGrantMaterial,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FakeStoreError;
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
pub(super) struct FakeResolver {
    pub(super) key: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FakeResolverError;
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

pub(super) fn sample_request() -> FeishuAuthRefreshRequest {
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

pub(super) fn sample_stored_material(
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

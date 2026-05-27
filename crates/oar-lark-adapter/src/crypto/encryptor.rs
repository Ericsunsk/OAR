use std::fmt;

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::Aes256Gcm;
use secrecy::{ExposeSecret, SecretBox};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::oauth::{FeishuGrantEncryptionInput, FeishuGrantEncryptor, FeishuGrantEnvelope};

use super::clock::{GrantTimeSource, SystemGrantClock};
use super::envelope::build_v1_envelope;

const MILLIS_PER_SECOND: u64 = 1000;

/// Local AEAD grant encryptor for the runtime adapter boundary.
///
/// This keeps plaintext Feishu tokens out of `oar-core` and stores only opaque
/// encrypted envelopes there. Production deployments may replace this with a KMS
/// backed `FeishuGrantEncryptor` without changing the core refresh contract.
pub struct AesGcmGrantEncryptor<C = SystemGrantClock> {
    key_id: String,
    key_material: SecretBox<[u8; 32]>,
    clock: C,
}

impl AesGcmGrantEncryptor<SystemGrantClock> {
    pub fn new(key_id: impl Into<String>, key_material: [u8; 32]) -> Self {
        Self::with_clock(key_id, key_material, SystemGrantClock)
    }
}

impl<C> AesGcmGrantEncryptor<C>
where
    C: GrantTimeSource,
{
    pub fn with_clock(key_id: impl Into<String>, key_material: [u8; 32], clock: C) -> Self {
        let key_material = SecretBox::new(Box::new(key_material));
        Self {
            key_id: key_id.into(),
            key_material,
            clock,
        }
    }

    fn seal_bytes(&self, plaintext: &[u8]) -> Result<Vec<u8>, AesGcmGrantEncryptorError> {
        let aead = Aes256Gcm::new_from_slice(self.key_material.expose_secret())
            .expect("32-byte key must always be valid for Aes256Gcm");
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = aead
            .encrypt(&nonce, plaintext)
            .map_err(|_| AesGcmGrantEncryptorError::EncryptionFailed)?;
        Ok(build_v1_envelope(&nonce, &ciphertext))
    }
}

impl<C> FeishuGrantEncryptor for AesGcmGrantEncryptor<C>
where
    C: GrantTimeSource,
{
    type Error = AesGcmGrantEncryptorError;

    fn encrypt(
        &mut self,
        input: FeishuGrantEncryptionInput,
    ) -> Result<FeishuGrantEnvelope, Self::Error> {
        let encrypted_primary = self.seal_bytes(input.access_token.expose_secret().as_bytes())?;
        let encrypted_renewal = self.seal_bytes(input.refresh_token.expose_secret().as_bytes())?;
        let refreshed_at_ms = self.clock.now_ms();
        let expires_at_ms = Some(
            refreshed_at_ms
                .saturating_add(input.expires_in_seconds.saturating_mul(MILLIS_PER_SECOND)),
        );

        let mut hasher = Sha256::new();
        hasher.update(self.key_id.as_bytes());
        hasher.update(&encrypted_renewal);
        let new_fingerprint = hex::encode(hasher.finalize());

        Ok(FeishuGrantEnvelope {
            encrypted_primary,
            encrypted_renewal,
            key_id: self.key_id.clone(),
            new_fingerprint,
            refreshed_at_ms,
            expires_at_ms,
        })
    }
}

impl<C> fmt::Debug for AesGcmGrantEncryptor<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AesGcmGrantEncryptor")
            .field("key_id", &"[REDACTED]")
            .field("key_material", &"[REDACTED]")
            .field("clock", &"[REDACTED]")
            .finish()
    }
}

impl<C> fmt::Display for AesGcmGrantEncryptor<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AesGcmGrantEncryptor([REDACTED])")
    }
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum AesGcmGrantEncryptorError {
    #[error("grant encryption failed")]
    EncryptionFailed,
}

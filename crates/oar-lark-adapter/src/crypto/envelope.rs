use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::Aes256Gcm;
use thiserror::Error;

pub(crate) const ENVELOPE_VERSION_V1: u8 = 1;
pub(crate) const NONCE_LEN_V1: usize = 12;

pub(crate) fn build_v1_envelope(nonce: &[u8], ciphertext: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + nonce.len() + ciphertext.len());
    out.push(ENVELOPE_VERSION_V1);
    out.push(NONCE_LEN_V1 as u8);
    out.extend_from_slice(nonce);
    out.extend_from_slice(ciphertext);
    out
}

pub(crate) fn decrypt_v1_envelope(
    key_material: &[u8; 32],
    envelope: &[u8],
) -> Result<Vec<u8>, AesGcmGrantDecryptError> {
    if envelope.len() < 2 + NONCE_LEN_V1 {
        return Err(AesGcmGrantDecryptError::InvalidEnvelope);
    }
    if envelope[0] != ENVELOPE_VERSION_V1 {
        return Err(AesGcmGrantDecryptError::InvalidEnvelope);
    }
    if envelope[1] as usize != NONCE_LEN_V1 {
        return Err(AesGcmGrantDecryptError::InvalidEnvelope);
    }
    let nonce = &envelope[2..(2 + NONCE_LEN_V1)];
    let ciphertext = &envelope[(2 + NONCE_LEN_V1)..];
    if ciphertext.is_empty() {
        return Err(AesGcmGrantDecryptError::InvalidEnvelope);
    }

    let aead = Aes256Gcm::new_from_slice(key_material)
        .expect("32-byte key must always be valid for Aes256Gcm");
    aead.decrypt(nonce.into(), ciphertext)
        .map_err(|_| AesGcmGrantDecryptError::DecryptionFailed)
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum AesGcmGrantDecryptError {
    #[error("grant envelope is invalid")]
    InvalidEnvelope,
    #[error("grant decryption failed")]
    DecryptionFailed,
}

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::Aes256Gcm;
use sha2::{Digest, Sha256};

use super::AgentModelSettingsError;

const SECRET_ENVELOPE_VERSION_V1: u8 = 1;
const SECRET_NONCE_LEN_V1: usize = 12;

pub(super) fn encrypt_secret(
    key_material: &[u8; 32],
    plaintext: &[u8],
) -> Result<Vec<u8>, AgentModelSettingsError> {
    let aead = Aes256Gcm::new_from_slice(key_material)
        .map_err(|_| AgentModelSettingsError::SecretCryptoFailed)?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = aead
        .encrypt(&nonce, plaintext)
        .map_err(|_| AgentModelSettingsError::SecretCryptoFailed)?;
    let mut envelope = Vec::with_capacity(2 + nonce.len() + ciphertext.len());
    envelope.push(SECRET_ENVELOPE_VERSION_V1);
    envelope.push(SECRET_NONCE_LEN_V1 as u8);
    envelope.extend_from_slice(&nonce);
    envelope.extend_from_slice(&ciphertext);
    Ok(envelope)
}

pub(super) fn decrypt_secret(
    key_material: &[u8; 32],
    envelope: &[u8],
) -> Result<String, AgentModelSettingsError> {
    if envelope.len() < 2 + SECRET_NONCE_LEN_V1
        || envelope[0] != SECRET_ENVELOPE_VERSION_V1
        || envelope[1] as usize != SECRET_NONCE_LEN_V1
    {
        return Err(AgentModelSettingsError::SecretCryptoFailed);
    }
    let nonce = &envelope[2..(2 + SECRET_NONCE_LEN_V1)];
    let ciphertext = &envelope[(2 + SECRET_NONCE_LEN_V1)..];
    if ciphertext.is_empty() {
        return Err(AgentModelSettingsError::SecretCryptoFailed);
    }
    let aead = Aes256Gcm::new_from_slice(key_material)
        .map_err(|_| AgentModelSettingsError::SecretCryptoFailed)?;
    let plaintext = aead
        .decrypt(nonce.into(), ciphertext)
        .map_err(|_| AgentModelSettingsError::SecretCryptoFailed)?;
    String::from_utf8(plaintext).map_err(|_| AgentModelSettingsError::SecretCryptoFailed)
}

pub(super) fn secret_fingerprint(key_material: &[u8; 32], key_id: &str, secret: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key_id.as_bytes());
    hasher.update(key_material);
    hasher.update(secret);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_envelope_roundtrips_without_plaintext() {
        let key = [9; 32];
        let encrypted = encrypt_secret(&key, b"sk-sensitive").expect("encrypt");

        assert!(!encrypted
            .windows(b"sk-sensitive".len())
            .any(|w| w == b"sk-sensitive"));
        assert_eq!(
            decrypt_secret(&key, &encrypted).expect("decrypt"),
            "sk-sensitive"
        );
        assert_eq!(
            secret_fingerprint(&key, "key-test", b"sk-sensitive"),
            secret_fingerprint(&key, "key-test", b"sk-sensitive")
        );
        assert_ne!(
            secret_fingerprint(&key, "key-test", b"sk-sensitive"),
            secret_fingerprint(&key, "key-test", b"sk-other")
        );
    }
}

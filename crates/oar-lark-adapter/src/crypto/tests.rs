use crate::crypto::{AesGcmGrantEncryptor, GrantTimeSource};
use crate::crypto::{ENVELOPE_VERSION_V1, NONCE_LEN_V1};
use crate::oauth::FeishuGrantEncryptionInput;
use crate::redaction::SecretString;
use crate::FeishuGrantEncryptor;

const ACCESS_TOKEN: &str = "uat-sensitive-access-token";
const REFRESH_TOKEN: &str = "urt-sensitive-refresh-token";

#[derive(Clone, Copy)]
struct FixedClock {
    now_ms: u64,
}

impl GrantTimeSource for FixedClock {
    fn now_ms(&self) -> u64 {
        self.now_ms
    }
}

#[test]
fn encrypt_success_outputs_non_empty_and_no_raw_tokens() {
    let mut encryptor =
        AesGcmGrantEncryptor::with_clock("key-1", [7; 32], FixedClock { now_ms: 10 });
    let input = sample_input(3600);

    let envelope = encryptor.encrypt(input).expect("encryption should succeed");
    assert!(!envelope.encrypted_primary.is_empty());
    assert!(!envelope.encrypted_renewal.is_empty());
    assert!(!contains_subslice(
        &envelope.encrypted_primary,
        ACCESS_TOKEN.as_bytes()
    ));
    assert!(!contains_subslice(
        &envelope.encrypted_renewal,
        REFRESH_TOKEN.as_bytes()
    ));
}

#[test]
fn encrypt_same_input_twice_has_different_nonce_and_ciphertext_with_valid_shape() {
    let mut encryptor =
        AesGcmGrantEncryptor::with_clock("key-2", [9; 32], FixedClock { now_ms: 1_000 });
    let input = sample_input(7200);

    let first = encryptor
        .encrypt(input.clone())
        .expect("first encryption should succeed");
    let second = encryptor
        .encrypt(input)
        .expect("second encryption should succeed");

    assert_ne!(first.encrypted_primary, second.encrypted_primary);
    assert_ne!(first.encrypted_renewal, second.encrypted_renewal);

    for encrypted in [
        first.encrypted_primary.as_slice(),
        first.encrypted_renewal.as_slice(),
        second.encrypted_primary.as_slice(),
        second.encrypted_renewal.as_slice(),
    ] {
        assert!(encrypted.len() > 2 + NONCE_LEN_V1);
        assert_eq!(encrypted[0], ENVELOPE_VERSION_V1);
        assert_eq!(encrypted[1], NONCE_LEN_V1 as u8);
    }
}

#[test]
fn expires_at_ms_uses_saturating_math() {
    let now = u64::MAX - 5;
    let mut encryptor =
        AesGcmGrantEncryptor::with_clock("key-3", [1; 32], FixedClock { now_ms: now });
    let envelope = encryptor
        .encrypt(sample_input(10))
        .expect("encryption should succeed");
    assert_eq!(envelope.refreshed_at_ms, now);
    assert_eq!(envelope.expires_at_ms, Some(u64::MAX));

    let mut encryptor_exact =
        AesGcmGrantEncryptor::with_clock("key-4", [2; 32], FixedClock { now_ms: 5000 });
    let exact = encryptor_exact
        .encrypt(sample_input(2))
        .expect("encryption should succeed");
    assert_eq!(exact.expires_at_ms, Some(7000));
}

#[test]
fn debug_and_display_do_not_leak_sensitive_material() {
    let mut encryptor =
        AesGcmGrantEncryptor::with_clock("kms-key-visible", [3; 32], FixedClock { now_ms: 100 });
    let envelope = encryptor
        .encrypt(sample_input(1))
        .expect("encryption should succeed");

    for rendered in [
        format!("{encryptor:?}"),
        encryptor.to_string(),
        format!("{envelope:?}"),
    ] {
        assert!(!rendered.contains("kms-key-visible"));
        assert!(!rendered.contains(ACCESS_TOKEN));
        assert!(!rendered.contains(REFRESH_TOKEN));
        assert!(!rendered.contains(&hex::encode([3; 32])));
        assert!(!rendered.contains(&hex::encode(&envelope.encrypted_primary)));
        assert!(!rendered.contains(&hex::encode(&envelope.encrypted_renewal)));
        assert!(!rendered.contains(&envelope.new_fingerprint));
    }
}

fn sample_input(expires_in_seconds: u64) -> FeishuGrantEncryptionInput {
    FeishuGrantEncryptionInput {
        grant_id: "grant-1".to_string(),
        tenant_id: "tenant-1".to_string(),
        expected_fingerprint: "fp-prev".to_string(),
        access_token: SecretString::new(ACCESS_TOKEN),
        refresh_token: SecretString::new(REFRESH_TOKEN),
        expires_in_seconds,
        refresh_token_expires_in_seconds: Some(60),
        token_type: Some("Bearer".to_string()),
        scope: Some("offline_access".to_string()),
    }
}

fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}

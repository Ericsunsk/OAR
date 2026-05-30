use crate::crypto::AesGcmGrantDecryptError;

#[test]
fn decrypt_helper_error_mapping_is_non_sensitive() {
    let err = AesGcmGrantDecryptError::InvalidEnvelope;
    let rendered = format!("{err}");
    assert!(!rendered.contains("token"));
    assert!(!rendered.contains("fingerprint"));
}

use crate::material::blob::{
    compose_encrypted_grant_blob, parse_encrypted_grant_blob,
    read_access_token_from_encrypted_grant, BlobParseError,
};
use crate::{AesGcmGrantEncryptor, FeishuGrantEncryptionInput, FeishuGrantEncryptor, SecretString};

#[test]
fn parse_rejects_invalid_shapes() {
    assert!(matches!(
        parse_encrypted_grant_blob(&[]),
        Err(BlobParseError::Invalid)
    ));
    assert!(matches!(
        parse_encrypted_grant_blob(&[0, 0, 0, 1, 7, 0, 0, 0]),
        Err(BlobParseError::Invalid)
    ));
}

#[test]
fn compose_roundtrip_extracts_segments() {
    let primary = vec![1, 2, 3, 4];
    let renewal = vec![8, 9];
    let blob = compose_encrypted_grant_blob(primary.clone(), renewal.clone());
    let (parsed_primary, parsed_renewal) =
        parse_encrypted_grant_blob(&blob).expect("blob should parse");
    assert_eq!(parsed_primary, primary.as_slice());
    assert_eq!(parsed_renewal, renewal.as_slice());
}

#[test]
fn read_access_token_decrypts_primary_material_only() {
    let key = [7_u8; 32];
    let mut encryptor = AesGcmGrantEncryptor::new("key-test", key);
    let envelope = encryptor
        .encrypt(FeishuGrantEncryptionInput {
            grant_id: "grant-1".to_string(),
            tenant_id: "tenant-1".to_string(),
            expected_fingerprint: "initial".to_string(),
            access_token: SecretString::new("access-token-sensitive"),
            refresh_token: SecretString::new("refresh-token-sensitive"),
            expires_in_seconds: 3600,
            refresh_token_expires_in_seconds: None,
            token_type: Some("Bearer".to_string()),
            scope: Some("okr:okr.progress:readonly".to_string()),
        })
        .expect("encrypt");
    let blob = compose_encrypted_grant_blob(envelope.encrypted_primary, envelope.encrypted_renewal);

    let token = read_access_token_from_encrypted_grant(&blob, key).expect("access token");

    assert_eq!(token.expose_secret(), "access-token-sensitive");
}

use super::helpers::{sample_request, sample_stored_material, FakeResolver, FakeStore};
use crate::material::blob::{compose_encrypted_grant_blob, parse_encrypted_grant_blob};
use crate::material::{AesGcmRefreshMaterialProvider, StoredFeishuGrantMaterial};

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

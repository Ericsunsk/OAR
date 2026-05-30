use super::helpers::{sample_request, sample_stored_material, FakeResolver, FakeStore};
use crate::material::blob::{compose_encrypted_grant_blob, parse_encrypted_grant_blob};
use crate::material::{AesGcmRefreshMaterialProvider, AesGcmRefreshMaterialProviderError};

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

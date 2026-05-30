use super::helpers::{sample_request, sample_stored_material, FakeResolver, FakeStore};
use crate::material::FeishuStoredRefreshMaterialProvider;
use crate::oauth::FeishuRefreshMaterialProvider;
use crate::redaction::SecretString;

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

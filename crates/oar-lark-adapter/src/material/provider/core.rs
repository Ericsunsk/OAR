use oar_core::lark::auth::types::FeishuAuthRefreshRequest;

use crate::crypto::decrypt_v1_envelope;
use crate::redaction::SecretString;

use super::errors::AesGcmRefreshMaterialProviderError;
use crate::material::blob::parse_encrypted_grant_blob;
use crate::material::types::{
    AesGcmKeyResolver, AsyncAesGcmKeyResolver, DecryptedFeishuGrantMaterial,
    StoredFeishuGrantMaterial,
};

pub(super) fn decrypt_and_validate<S, K>(
    request: &FeishuAuthRefreshRequest,
    stored: StoredFeishuGrantMaterial,
    key_resolver: &mut K,
) -> Result<DecryptedFeishuGrantMaterial, AesGcmRefreshMaterialProviderError<S, K::Error>>
where
    K: AesGcmKeyResolver,
{
    validate_stored_request(request, &stored)?;

    let key = key_resolver
        .key_for(&stored.oauth_grant_key_id)
        .map_err(AesGcmRefreshMaterialProviderError::KeyResolver)?;
    decrypt_renewal_material(&key, stored)
}

pub(super) async fn decrypt_and_validate_async<S, K>(
    request: &FeishuAuthRefreshRequest,
    stored: StoredFeishuGrantMaterial,
    key_resolver: &mut K,
) -> Result<DecryptedFeishuGrantMaterial, AesGcmRefreshMaterialProviderError<S, K::Error>>
where
    K: AsyncAesGcmKeyResolver,
{
    validate_stored_request(request, &stored)?;

    let key = key_resolver
        .key_for(&stored.oauth_grant_key_id)
        .await
        .map_err(AesGcmRefreshMaterialProviderError::KeyResolver)?;
    decrypt_renewal_material(&key, stored)
}

fn validate_stored_request<S, K>(
    request: &FeishuAuthRefreshRequest,
    stored: &StoredFeishuGrantMaterial,
) -> Result<(), AesGcmRefreshMaterialProviderError<S, K>> {
    if request.grant_id != stored.grant_id || request.tenant_id != stored.tenant_id {
        return Err(AesGcmRefreshMaterialProviderError::GrantMismatch);
    }
    if request.expected_fingerprint != stored.oauth_grant_fingerprint {
        return Err(AesGcmRefreshMaterialProviderError::FingerprintMismatch);
    }
    Ok(())
}

fn decrypt_renewal_material<S, K>(
    key: &[u8; 32],
    stored: StoredFeishuGrantMaterial,
) -> Result<DecryptedFeishuGrantMaterial, AesGcmRefreshMaterialProviderError<S, K>> {
    let (_, renewal) = parse_encrypted_grant_blob(&stored.encrypted_oauth_grant)
        .map_err(|_| AesGcmRefreshMaterialProviderError::MalformedGrantMaterial)?;
    let refresh_token = decrypt_v1_envelope(&key, renewal)
        .map_err(|_| AesGcmRefreshMaterialProviderError::DecryptFailed)?;
    let refresh_token = String::from_utf8(refresh_token)
        .map_err(|_| AesGcmRefreshMaterialProviderError::DecryptFailed)?;

    Ok(DecryptedFeishuGrantMaterial {
        refresh_token: SecretString::new(refresh_token),
        scope: stored.scope,
    })
}

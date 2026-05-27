mod core;
mod errors;

use std::fmt;

use async_trait::async_trait;
use oar_core::lark::auth::types::FeishuAuthRefreshRequest;

use crate::credentials::{
    AsyncFeishuAppCredentialProvider, FeishuAppCredential, FeishuAppCredentialProvider,
};
use crate::oauth::{
    AsyncFeishuRefreshMaterialProvider, FeishuRefreshMaterial, FeishuRefreshMaterialProvider,
};

use self::core::{decrypt_and_validate, decrypt_and_validate_async};
pub use self::errors::{
    AesGcmRefreshMaterialProviderError, FeishuStoredRefreshMaterialProviderError,
};
use crate::material::types::{
    AesGcmKeyResolver, AsyncAesGcmKeyResolver, AsyncFeishuGrantMaterialStore,
    FeishuGrantMaterialStore,
};

pub struct AesGcmRefreshMaterialProvider<S, K> {
    store: S,
    key_resolver: K,
}

impl<S, K> AesGcmRefreshMaterialProvider<S, K> {
    pub fn new(store: S, key_resolver: K) -> Self {
        Self {
            store,
            key_resolver,
        }
    }

    pub fn decrypted_grant_material(
        &mut self,
        request: &FeishuAuthRefreshRequest,
    ) -> Result<
        crate::material::types::DecryptedFeishuGrantMaterial,
        AesGcmRefreshMaterialProviderError<S::Error, K::Error>,
    >
    where
        S: FeishuGrantMaterialStore,
        K: AesGcmKeyResolver,
    {
        let stored = self
            .store
            .load(request)
            .map_err(AesGcmRefreshMaterialProviderError::Store)?;
        decrypt_and_validate(request, stored, &mut self.key_resolver)
    }

    pub async fn decrypted_grant_material_async(
        &mut self,
        request: &FeishuAuthRefreshRequest,
    ) -> Result<
        crate::material::types::DecryptedFeishuGrantMaterial,
        AesGcmRefreshMaterialProviderError<S::Error, K::Error>,
    >
    where
        S: AsyncFeishuGrantMaterialStore,
        K: AsyncAesGcmKeyResolver,
    {
        let stored = self
            .store
            .load(request)
            .await
            .map_err(AesGcmRefreshMaterialProviderError::Store)?;
        decrypt_and_validate_async(request, stored, &mut self.key_resolver).await
    }
}

impl<S, K> fmt::Debug for AesGcmRefreshMaterialProvider<S, K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AesGcmRefreshMaterialProvider")
            .field("store", &"[REDACTED]")
            .field("key_resolver", &"[REDACTED]")
            .finish()
    }
}

impl<S, K> fmt::Display for AesGcmRefreshMaterialProvider<S, K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AesGcmRefreshMaterialProvider([REDACTED])")
    }
}

pub struct FeishuStoredRefreshMaterialProvider<S, K, C> {
    grant_provider: AesGcmRefreshMaterialProvider<S, K>,
    credential_provider: C,
}

impl<S, K, C> FeishuStoredRefreshMaterialProvider<S, K, C> {
    pub fn new(store: S, key_resolver: K, credential_provider: C) -> Self {
        Self {
            grant_provider: AesGcmRefreshMaterialProvider::new(store, key_resolver),
            credential_provider,
        }
    }
}

impl<S, K, C> fmt::Debug for FeishuStoredRefreshMaterialProvider<S, K, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuStoredRefreshMaterialProvider")
            .field("grant_provider", &"[REDACTED]")
            .field("credential_provider", &"[REDACTED]")
            .finish()
    }
}

impl<S, K, C> fmt::Display for FeishuStoredRefreshMaterialProvider<S, K, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FeishuStoredRefreshMaterialProvider([REDACTED])")
    }
}

impl<S, K, C> FeishuRefreshMaterialProvider for FeishuStoredRefreshMaterialProvider<S, K, C>
where
    S: FeishuGrantMaterialStore,
    K: AesGcmKeyResolver,
    C: FeishuAppCredentialProvider,
{
    type Error = FeishuStoredRefreshMaterialProviderError<
        AesGcmRefreshMaterialProviderError<S::Error, K::Error>,
        C::Error,
    >;

    fn refresh_material(
        &mut self,
        request: &FeishuAuthRefreshRequest,
    ) -> Result<FeishuRefreshMaterial, Self::Error> {
        let grant_material = self
            .grant_provider
            .decrypted_grant_material(request)
            .map_err(FeishuStoredRefreshMaterialProviderError::Grant)?;
        let FeishuAppCredential {
            client_id,
            client_secret,
        } = self
            .credential_provider
            .credentials(request)
            .map_err(FeishuStoredRefreshMaterialProviderError::Credential)?;

        Ok(FeishuRefreshMaterial {
            client_id,
            client_secret,
            refresh_token: grant_material.refresh_token,
            scope: grant_material.scope,
        })
    }
}

#[async_trait(?Send)]
impl<S, K, C> AsyncFeishuRefreshMaterialProvider for FeishuStoredRefreshMaterialProvider<S, K, C>
where
    S: AsyncFeishuGrantMaterialStore,
    K: AsyncAesGcmKeyResolver,
    C: AsyncFeishuAppCredentialProvider,
{
    type Error = FeishuStoredRefreshMaterialProviderError<
        AesGcmRefreshMaterialProviderError<S::Error, K::Error>,
        C::Error,
    >;

    async fn refresh_material(
        &mut self,
        request: &FeishuAuthRefreshRequest,
    ) -> Result<FeishuRefreshMaterial, Self::Error> {
        let grant_material = self
            .grant_provider
            .decrypted_grant_material_async(request)
            .await
            .map_err(FeishuStoredRefreshMaterialProviderError::Grant)?;
        let FeishuAppCredential {
            client_id,
            client_secret,
        } = self
            .credential_provider
            .credentials(request)
            .await
            .map_err(FeishuStoredRefreshMaterialProviderError::Credential)?;

        Ok(FeishuRefreshMaterial {
            client_id,
            client_secret,
            refresh_token: grant_material.refresh_token,
            scope: grant_material.scope,
        })
    }
}

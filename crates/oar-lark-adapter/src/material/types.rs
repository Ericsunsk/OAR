use std::fmt;

use async_trait::async_trait;
use oar_core::lark::auth::types::FeishuAuthRefreshRequest;

use crate::redaction::SecretString;

#[derive(Clone)]
pub struct StoredFeishuGrantMaterial {
    pub grant_id: String,
    pub tenant_id: String,
    pub encrypted_oauth_grant: Vec<u8>,
    pub oauth_grant_key_id: String,
    pub oauth_grant_fingerprint: String,
    pub scope: Option<String>,
}

impl fmt::Debug for StoredFeishuGrantMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StoredFeishuGrantMaterial")
            .field("grant_id", &self.grant_id)
            .field("tenant_id", &self.tenant_id)
            .field("encrypted_oauth_grant", &"[REDACTED]")
            .field("oauth_grant_key_id", &"[REDACTED]")
            .field("oauth_grant_fingerprint", &"[REDACTED]")
            .field("scope", &self.scope)
            .finish()
    }
}

pub trait FeishuGrantMaterialStore {
    type Error;

    fn load(
        &mut self,
        request: &FeishuAuthRefreshRequest,
    ) -> Result<StoredFeishuGrantMaterial, Self::Error>;
}

#[async_trait]
pub trait AsyncFeishuGrantMaterialStore {
    type Error;

    async fn load(
        &mut self,
        request: &FeishuAuthRefreshRequest,
    ) -> Result<StoredFeishuGrantMaterial, Self::Error>;
}

#[async_trait]
impl<T> AsyncFeishuGrantMaterialStore for T
where
    T: FeishuGrantMaterialStore + Send,
{
    type Error = T::Error;

    async fn load(
        &mut self,
        request: &FeishuAuthRefreshRequest,
    ) -> Result<StoredFeishuGrantMaterial, Self::Error> {
        FeishuGrantMaterialStore::load(self, request)
    }
}

pub trait AesGcmKeyResolver {
    type Error;

    fn key_for(&mut self, key_id: &str) -> Result<[u8; 32], Self::Error>;
}

#[async_trait]
pub trait AsyncAesGcmKeyResolver {
    type Error;

    async fn key_for(&mut self, key_id: &str) -> Result<[u8; 32], Self::Error>;
}

#[async_trait]
impl<T> AsyncAesGcmKeyResolver for T
where
    T: AesGcmKeyResolver + Send,
{
    type Error = T::Error;

    async fn key_for(&mut self, key_id: &str) -> Result<[u8; 32], Self::Error> {
        AesGcmKeyResolver::key_for(self, key_id)
    }
}

#[derive(Clone)]
pub struct DecryptedFeishuGrantMaterial {
    pub refresh_token: SecretString,
    pub scope: Option<String>,
}

impl fmt::Debug for DecryptedFeishuGrantMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecryptedFeishuGrantMaterial")
            .field("refresh_token", &"[REDACTED]")
            .field("scope", &self.scope)
            .finish()
    }
}

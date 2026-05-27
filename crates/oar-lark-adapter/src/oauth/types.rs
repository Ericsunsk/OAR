use std::fmt;

use async_trait::async_trait;
use oar_core::lark::auth::types::FeishuAuthRefreshRequest;

use crate::redaction::SecretString;

pub trait FeishuRefreshMaterialProvider {
    type Error;

    fn refresh_material(
        &mut self,
        request: &FeishuAuthRefreshRequest,
    ) -> Result<FeishuRefreshMaterial, Self::Error>;
}

#[async_trait(?Send)]
pub trait AsyncFeishuRefreshMaterialProvider {
    type Error;

    async fn refresh_material(
        &mut self,
        request: &FeishuAuthRefreshRequest,
    ) -> Result<FeishuRefreshMaterial, Self::Error>;
}

pub trait FeishuGrantEncryptor {
    type Error;

    fn encrypt(
        &mut self,
        input: FeishuGrantEncryptionInput,
    ) -> Result<FeishuGrantEnvelope, Self::Error>;
}

#[derive(Clone)]
pub struct FeishuRefreshMaterial {
    pub client_id: String,
    pub client_secret: SecretString,
    pub refresh_token: SecretString,
    pub scope: Option<String>,
}

impl fmt::Debug for FeishuRefreshMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuRefreshMaterial")
            .field("client_id", &self.client_id)
            .field("client_credential", &"[REDACTED]")
            .field("renewal_credential", &"[REDACTED]")
            .field("scope", &self.scope)
            .finish()
    }
}

#[derive(Clone)]
pub struct FeishuGrantEncryptionInput {
    pub grant_id: String,
    pub tenant_id: String,
    pub expected_fingerprint: String,
    pub access_token: SecretString,
    pub refresh_token: SecretString,
    pub expires_in_seconds: u64,
    pub refresh_token_expires_in_seconds: Option<u64>,
    pub token_type: Option<String>,
    pub scope: Option<String>,
}

impl fmt::Debug for FeishuGrantEncryptionInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuGrantEncryptionInput")
            .field("grant_id", &self.grant_id)
            .field("tenant_id", &self.tenant_id)
            .field("cas_marker", &"[REDACTED]")
            .field("primary_credential", &"[REDACTED]")
            .field("renewal_credential", &"[REDACTED]")
            .field("expires_in_seconds", &self.expires_in_seconds)
            .field(
                "renewal_expires_in_seconds",
                &self.refresh_token_expires_in_seconds,
            )
            .field("credential_type", &"[REDACTED]")
            .field("scope", &self.scope)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct FeishuGrantEnvelope {
    pub encrypted_primary: Vec<u8>,
    pub encrypted_renewal: Vec<u8>,
    pub key_id: String,
    pub new_fingerprint: String,
    pub refreshed_at_ms: u64,
    pub expires_at_ms: Option<u64>,
}

impl fmt::Debug for FeishuGrantEnvelope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuGrantEnvelope")
            .field("encrypted_primary", &"[REDACTED]")
            .field("encrypted_renewal", &"[REDACTED]")
            .field("key_id", &"[REDACTED]")
            .field("new_fingerprint", &"[REDACTED]")
            .field("refreshed_at_ms", &self.refreshed_at_ms)
            .field("expires_at_ms", &self.expires_at_ms)
            .finish()
    }
}

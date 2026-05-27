use std::fmt;

use async_trait::async_trait;
use oar_core::lark::auth::types::FeishuAuthRefreshRequest;

use crate::redaction::SecretString;

#[derive(Clone)]
pub struct FeishuAppCredential {
    pub client_id: String,
    pub client_secret: SecretString,
}

impl fmt::Debug for FeishuAppCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuAppCredential")
            .field("client_id", &self.client_id)
            .field("client_secret", &"[REDACTED]")
            .finish()
    }
}

pub trait FeishuAppCredentialProvider {
    type Error;

    fn credentials(
        &mut self,
        request: &FeishuAuthRefreshRequest,
    ) -> Result<FeishuAppCredential, Self::Error>;
}

#[async_trait]
pub trait AsyncFeishuAppCredentialProvider {
    type Error;

    async fn credentials(
        &mut self,
        request: &FeishuAuthRefreshRequest,
    ) -> Result<FeishuAppCredential, Self::Error>;
}

#[async_trait]
impl<T> AsyncFeishuAppCredentialProvider for T
where
    T: FeishuAppCredentialProvider + Send,
{
    type Error = T::Error;

    async fn credentials(
        &mut self,
        request: &FeishuAuthRefreshRequest,
    ) -> Result<FeishuAppCredential, Self::Error> {
        FeishuAppCredentialProvider::credentials(self, request)
    }
}

#[derive(Clone)]
pub struct StaticFeishuAppCredentialProvider {
    credential: FeishuAppCredential,
}

impl StaticFeishuAppCredentialProvider {
    pub fn new(client_id: impl Into<String>, client_secret: SecretString) -> Self {
        Self {
            credential: FeishuAppCredential {
                client_id: client_id.into(),
                client_secret,
            },
        }
    }
}

impl fmt::Debug for StaticFeishuAppCredentialProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StaticFeishuAppCredentialProvider")
            .field("credential", &self.credential)
            .finish()
    }
}

impl FeishuAppCredentialProvider for StaticFeishuAppCredentialProvider {
    type Error = std::convert::Infallible;

    fn credentials(
        &mut self,
        _request: &FeishuAuthRefreshRequest,
    ) -> Result<FeishuAppCredential, Self::Error> {
        Ok(self.credential.clone())
    }
}

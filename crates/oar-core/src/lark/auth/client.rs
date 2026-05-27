use std::fmt;

use async_trait::async_trait;

use super::adapter::FeishuAuthRefreshClient;
use super::parser::parse_feishu_auth_refresh_response;
use super::types::{FeishuAuthRefreshRequest, FeishuAuthRefreshResponse};

#[derive(Clone, PartialEq, Eq)]
pub struct FeishuAuthRefreshRawEnvelope {
    payload: String,
}

impl FeishuAuthRefreshRawEnvelope {
    pub fn new(payload: impl Into<String>) -> Self {
        Self {
            payload: payload.into(),
        }
    }

    pub fn byte_len(&self) -> usize {
        self.payload.len()
    }

    fn payload(&self) -> &str {
        &self.payload
    }
}

impl fmt::Debug for FeishuAuthRefreshRawEnvelope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuAuthRefreshRawEnvelope")
            .field("payload", &"[REDACTED]")
            .field("byte_len", &self.byte_len())
            .finish()
    }
}

pub trait FeishuAuthRefreshTransport {
    type Error;

    fn execute(
        &mut self,
        request: &FeishuAuthRefreshRequest,
    ) -> Result<FeishuAuthRefreshRawEnvelope, Self::Error>;
}

#[async_trait(?Send)]
pub trait AsyncFeishuAuthRefreshTransport {
    type Error;

    async fn execute(
        &mut self,
        request: &FeishuAuthRefreshRequest,
    ) -> Result<FeishuAuthRefreshRawEnvelope, Self::Error>;
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct FeishuAuthRefreshSafeClientConfig {
    pub max_response_bytes: usize,
}

impl Default for FeishuAuthRefreshSafeClientConfig {
    fn default() -> Self {
        Self {
            max_response_bytes: 64 * 1024,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct FeishuAuthRefreshSafeClient<T> {
    transport: T,
    config: FeishuAuthRefreshSafeClientConfig,
}

impl<T> FeishuAuthRefreshSafeClient<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            config: FeishuAuthRefreshSafeClientConfig::default(),
        }
    }

    pub fn with_config(transport: T, config: FeishuAuthRefreshSafeClientConfig) -> Self {
        Self { transport, config }
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    pub fn config(&self) -> FeishuAuthRefreshSafeClientConfig {
        self.config
    }
}

impl<T> fmt::Debug for FeishuAuthRefreshSafeClient<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuAuthRefreshSafeClient")
            .field("transport", &"[REDACTED]")
            .field("config", &self.config)
            .finish()
    }
}

impl fmt::Debug for FeishuAuthRefreshSafeClientConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuAuthRefreshSafeClientConfig")
            .field("max_response_bytes", &self.max_response_bytes)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum FeishuAuthRefreshClientError {
    Transport,
    OversizedResponse { max_response_bytes: usize },
    Parse,
}

impl FeishuAuthRefreshClientError {
    pub fn classify(&self) -> &'static str {
        match self {
            FeishuAuthRefreshClientError::Transport => "transport",
            FeishuAuthRefreshClientError::OversizedResponse { .. } => "oversized_response",
            FeishuAuthRefreshClientError::Parse => "parse",
        }
    }
}

impl fmt::Debug for FeishuAuthRefreshClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FeishuAuthRefreshClientError::Transport => {
                write!(f, "FeishuAuthRefreshClientError(transport)")
            }
            FeishuAuthRefreshClientError::OversizedResponse { max_response_bytes } => write!(
                f,
                "FeishuAuthRefreshClientError(oversized_response max={}B)",
                max_response_bytes
            ),
            FeishuAuthRefreshClientError::Parse => {
                write!(f, "FeishuAuthRefreshClientError(parse)")
            }
        }
    }
}

impl fmt::Display for FeishuAuthRefreshClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FeishuAuthRefreshClientError::Transport => {
                write!(f, "lark auth refresh transport failed")
            }
            FeishuAuthRefreshClientError::OversizedResponse { max_response_bytes } => write!(
                f,
                "lark auth refresh response exceeded {} bytes",
                max_response_bytes
            ),
            FeishuAuthRefreshClientError::Parse => {
                write!(f, "lark auth refresh response parse failed")
            }
        }
    }
}

impl std::error::Error for FeishuAuthRefreshClientError {}

impl<T> FeishuAuthRefreshClient for FeishuAuthRefreshSafeClient<T>
where
    T: FeishuAuthRefreshTransport,
{
    type Error = FeishuAuthRefreshClientError;

    fn refresh(
        &mut self,
        request: &FeishuAuthRefreshRequest,
    ) -> Result<FeishuAuthRefreshResponse, Self::Error> {
        let envelope = self
            .transport
            .execute(request)
            .map_err(|_| FeishuAuthRefreshClientError::Transport)?;

        if envelope.byte_len() > self.config.max_response_bytes {
            return Err(FeishuAuthRefreshClientError::OversizedResponse {
                max_response_bytes: self.config.max_response_bytes,
            });
        }

        parse_feishu_auth_refresh_response(envelope.payload())
            .map_err(|_| FeishuAuthRefreshClientError::Parse)
    }
}

impl<T> FeishuAuthRefreshSafeClient<T>
where
    T: AsyncFeishuAuthRefreshTransport,
{
    pub async fn refresh_async(
        &mut self,
        request: &FeishuAuthRefreshRequest,
    ) -> Result<FeishuAuthRefreshResponse, FeishuAuthRefreshClientError> {
        let envelope = self
            .transport
            .execute(request)
            .await
            .map_err(|_| FeishuAuthRefreshClientError::Transport)?;

        if envelope.byte_len() > self.config.max_response_bytes {
            return Err(FeishuAuthRefreshClientError::OversizedResponse {
                max_response_bytes: self.config.max_response_bytes,
            });
        }

        parse_feishu_auth_refresh_response(envelope.payload())
            .map_err(|_| FeishuAuthRefreshClientError::Parse)
    }
}

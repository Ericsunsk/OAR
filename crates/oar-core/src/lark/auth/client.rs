use std::fmt;

use async_trait::async_trait;

use super::adapter::LarkAuthRefreshClient;
use super::parser::parse_lark_auth_refresh_response;
use super::types::{LarkAuthRefreshRequest, LarkAuthRefreshResponse};

#[derive(Clone, PartialEq, Eq)]
pub struct LarkAuthRefreshRawEnvelope {
    payload: String,
}

impl LarkAuthRefreshRawEnvelope {
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

impl fmt::Debug for LarkAuthRefreshRawEnvelope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LarkAuthRefreshRawEnvelope")
            .field("payload", &"[REDACTED]")
            .field("byte_len", &self.byte_len())
            .finish()
    }
}

pub trait LarkAuthRefreshTransport {
    type Error;

    fn execute(
        &mut self,
        request: &LarkAuthRefreshRequest,
    ) -> Result<LarkAuthRefreshRawEnvelope, Self::Error>;
}

#[async_trait(?Send)]
pub trait AsyncLarkAuthRefreshTransport {
    type Error;

    async fn execute(
        &mut self,
        request: &LarkAuthRefreshRequest,
    ) -> Result<LarkAuthRefreshRawEnvelope, Self::Error>;
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct LarkAuthRefreshSafeClientConfig {
    pub max_response_bytes: usize,
}

impl Default for LarkAuthRefreshSafeClientConfig {
    fn default() -> Self {
        Self {
            max_response_bytes: 64 * 1024,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct LarkAuthRefreshSafeClient<T> {
    transport: T,
    config: LarkAuthRefreshSafeClientConfig,
}

impl<T> LarkAuthRefreshSafeClient<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            config: LarkAuthRefreshSafeClientConfig::default(),
        }
    }

    pub fn with_config(transport: T, config: LarkAuthRefreshSafeClientConfig) -> Self {
        Self { transport, config }
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    pub fn config(&self) -> LarkAuthRefreshSafeClientConfig {
        self.config
    }
}

impl<T> fmt::Debug for LarkAuthRefreshSafeClient<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LarkAuthRefreshSafeClient")
            .field("transport", &"[REDACTED]")
            .field("config", &self.config)
            .finish()
    }
}

impl fmt::Debug for LarkAuthRefreshSafeClientConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LarkAuthRefreshSafeClientConfig")
            .field("max_response_bytes", &self.max_response_bytes)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum LarkAuthRefreshClientError {
    Transport,
    OversizedResponse { max_response_bytes: usize },
    Parse,
}

impl LarkAuthRefreshClientError {
    pub fn classify(&self) -> &'static str {
        match self {
            LarkAuthRefreshClientError::Transport => "transport",
            LarkAuthRefreshClientError::OversizedResponse { .. } => "oversized_response",
            LarkAuthRefreshClientError::Parse => "parse",
        }
    }
}

impl fmt::Debug for LarkAuthRefreshClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LarkAuthRefreshClientError::Transport => {
                write!(f, "LarkAuthRefreshClientError(transport)")
            }
            LarkAuthRefreshClientError::OversizedResponse { max_response_bytes } => write!(
                f,
                "LarkAuthRefreshClientError(oversized_response max={}B)",
                max_response_bytes
            ),
            LarkAuthRefreshClientError::Parse => {
                write!(f, "LarkAuthRefreshClientError(parse)")
            }
        }
    }
}

impl fmt::Display for LarkAuthRefreshClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LarkAuthRefreshClientError::Transport => {
                write!(f, "lark auth refresh transport failed")
            }
            LarkAuthRefreshClientError::OversizedResponse { max_response_bytes } => write!(
                f,
                "lark auth refresh response exceeded {} bytes",
                max_response_bytes
            ),
            LarkAuthRefreshClientError::Parse => {
                write!(f, "lark auth refresh response parse failed")
            }
        }
    }
}

impl std::error::Error for LarkAuthRefreshClientError {}

impl<T> LarkAuthRefreshClient for LarkAuthRefreshSafeClient<T>
where
    T: LarkAuthRefreshTransport,
{
    type Error = LarkAuthRefreshClientError;

    fn refresh(
        &mut self,
        request: &LarkAuthRefreshRequest,
    ) -> Result<LarkAuthRefreshResponse, Self::Error> {
        let envelope = self
            .transport
            .execute(request)
            .map_err(|_| LarkAuthRefreshClientError::Transport)?;

        if envelope.byte_len() > self.config.max_response_bytes {
            return Err(LarkAuthRefreshClientError::OversizedResponse {
                max_response_bytes: self.config.max_response_bytes,
            });
        }

        parse_lark_auth_refresh_response(envelope.payload())
            .map_err(|_| LarkAuthRefreshClientError::Parse)
    }
}

impl<T> LarkAuthRefreshSafeClient<T>
where
    T: AsyncLarkAuthRefreshTransport,
{
    pub async fn refresh_async(
        &mut self,
        request: &LarkAuthRefreshRequest,
    ) -> Result<LarkAuthRefreshResponse, LarkAuthRefreshClientError> {
        let envelope = self
            .transport
            .execute(request)
            .await
            .map_err(|_| LarkAuthRefreshClientError::Transport)?;

        if envelope.byte_len() > self.config.max_response_bytes {
            return Err(LarkAuthRefreshClientError::OversizedResponse {
                max_response_bytes: self.config.max_response_bytes,
            });
        }

        parse_lark_auth_refresh_response(envelope.payload())
            .map_err(|_| LarkAuthRefreshClientError::Parse)
    }
}

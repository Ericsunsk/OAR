use std::fmt;

use crate::oauth::HttpClientFailure;

#[derive(Clone, PartialEq, Eq)]
pub enum FeishuOkrReadError {
    InvalidRequest,
    Unauthorized,
    Forbidden,
    UpstreamClient,
    UpstreamTransient,
    Transport,
    OversizedResponse,
    InvalidJson,
    ApiFailure,
}

impl fmt::Debug for FeishuOkrReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            FeishuOkrReadError::InvalidRequest => "invalid_request",
            FeishuOkrReadError::Unauthorized => "unauthorized",
            FeishuOkrReadError::Forbidden => "forbidden",
            FeishuOkrReadError::UpstreamClient => "upstream_client",
            FeishuOkrReadError::UpstreamTransient => "upstream_transient",
            FeishuOkrReadError::Transport => "transport",
            FeishuOkrReadError::OversizedResponse => "oversized_response",
            FeishuOkrReadError::InvalidJson => "invalid_json",
            FeishuOkrReadError::ApiFailure => "api_failure",
        };
        write!(f, "FeishuOkrReadError({label})")
    }
}

impl fmt::Display for FeishuOkrReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            FeishuOkrReadError::InvalidRequest => "invalid okr request",
            FeishuOkrReadError::Unauthorized => "unauthorized",
            FeishuOkrReadError::Forbidden => "forbidden",
            FeishuOkrReadError::UpstreamClient => "upstream request failed",
            FeishuOkrReadError::UpstreamTransient => "temporarily unavailable",
            FeishuOkrReadError::Transport => "feishu okr transport failed",
            FeishuOkrReadError::OversizedResponse => "feishu okr response too large",
            FeishuOkrReadError::InvalidJson => "feishu okr invalid json response",
            FeishuOkrReadError::ApiFailure => "feishu okr api returned failure",
        };
        f.write_str(message)
    }
}

impl std::error::Error for FeishuOkrReadError {}

impl From<HttpClientFailure> for FeishuOkrReadError {
    fn from(value: HttpClientFailure) -> Self {
        match value {
            HttpClientFailure::Transport => FeishuOkrReadError::Transport,
            HttpClientFailure::OversizedResponse { .. } => FeishuOkrReadError::OversizedResponse,
        }
    }
}

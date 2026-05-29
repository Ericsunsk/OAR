use std::fmt;

use crate::oauth::HttpClientFailure;

#[derive(Clone, PartialEq, Eq)]
pub enum FeishuTaskReadError {
    InvalidSourceRef,
    Unauthorized,
    Forbidden,
    NotFound,
    UpstreamClient,
    UpstreamTransient,
    Transport,
    OversizedResponse,
    InvalidJson,
    ApiFailure,
}

impl fmt::Debug for FeishuTaskReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            FeishuTaskReadError::InvalidSourceRef => "invalid_source_ref",
            FeishuTaskReadError::Unauthorized => "unauthorized",
            FeishuTaskReadError::Forbidden => "forbidden",
            FeishuTaskReadError::NotFound => "not_found",
            FeishuTaskReadError::UpstreamClient => "upstream_client",
            FeishuTaskReadError::UpstreamTransient => "upstream_transient",
            FeishuTaskReadError::Transport => "transport",
            FeishuTaskReadError::OversizedResponse => "oversized_response",
            FeishuTaskReadError::InvalidJson => "invalid_json",
            FeishuTaskReadError::ApiFailure => "api_failure",
        };
        write!(f, "FeishuTaskReadError({label})")
    }
}

impl fmt::Display for FeishuTaskReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            FeishuTaskReadError::InvalidSourceRef => "invalid task source reference",
            FeishuTaskReadError::Unauthorized => "unauthorized",
            FeishuTaskReadError::Forbidden => "forbidden",
            FeishuTaskReadError::NotFound => "task not found",
            FeishuTaskReadError::UpstreamClient => "upstream request failed",
            FeishuTaskReadError::UpstreamTransient => "temporarily unavailable",
            FeishuTaskReadError::Transport => "feishu task transport failed",
            FeishuTaskReadError::OversizedResponse => "feishu task response too large",
            FeishuTaskReadError::InvalidJson => "feishu task invalid json response",
            FeishuTaskReadError::ApiFailure => "feishu task api returned failure",
        };
        f.write_str(message)
    }
}

impl std::error::Error for FeishuTaskReadError {}

impl From<HttpClientFailure> for FeishuTaskReadError {
    fn from(value: HttpClientFailure) -> Self {
        match value {
            HttpClientFailure::Transport => FeishuTaskReadError::Transport,
            HttpClientFailure::OversizedResponse { .. } => FeishuTaskReadError::OversizedResponse,
        }
    }
}

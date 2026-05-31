use std::fmt;

use crate::oauth::HttpClientFailure;

#[derive(Clone, PartialEq, Eq)]
pub enum FeishuMinutesReadError {
    InvalidSourceRef,
    Unauthorized,
    Forbidden,
    NotFound,
    UpstreamClient,
    UpstreamTransient,
    Transport,
    ApiFailure,
    InvalidJson,
    InvalidRequest,
    OversizedResponse,
}

impl fmt::Debug for FeishuMinutesReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::InvalidSourceRef => "invalid_source_ref",
            Self::Unauthorized => "unauthorized",
            Self::Forbidden => "forbidden",
            Self::NotFound => "not_found",
            Self::UpstreamClient => "upstream_client",
            Self::UpstreamTransient => "upstream_transient",
            Self::Transport => "transport",
            Self::ApiFailure => "api_failure",
            Self::InvalidJson => "invalid_json",
            Self::InvalidRequest => "invalid_request",
            Self::OversizedResponse => "oversized_response",
        };
        write!(f, "FeishuMinutesReadError({label})")
    }
}

impl fmt::Display for FeishuMinutesReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidSourceRef => "invalid feishu minutes source reference",
            Self::Unauthorized => "feishu minutes token unauthorized",
            Self::Forbidden => "feishu minutes permission denied",
            Self::NotFound => "feishu minutes resource not found",
            Self::UpstreamClient => "feishu minutes upstream rejected request",
            Self::UpstreamTransient => "feishu minutes upstream transient failure",
            Self::Transport => "feishu minutes transport failed",
            Self::ApiFailure => "feishu minutes api failure",
            Self::InvalidJson => "feishu minutes invalid json",
            Self::InvalidRequest => "feishu minutes invalid request",
            Self::OversizedResponse => "feishu minutes response too large",
        };
        f.write_str(message)
    }
}

impl std::error::Error for FeishuMinutesReadError {}

impl From<HttpClientFailure> for FeishuMinutesReadError {
    fn from(value: HttpClientFailure) -> Self {
        match value {
            HttpClientFailure::Transport => Self::Transport,
            HttpClientFailure::OversizedResponse { .. } => Self::OversizedResponse,
        }
    }
}

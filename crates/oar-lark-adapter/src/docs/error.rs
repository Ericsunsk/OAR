use std::fmt;

use crate::oauth::HttpClientFailure;

#[derive(Clone, PartialEq, Eq)]
pub enum FeishuDocReadError {
    InvalidSourceRef,
    UnsupportedDocumentType,
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

impl fmt::Debug for FeishuDocReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::InvalidSourceRef => "invalid_source_ref",
            Self::UnsupportedDocumentType => "unsupported_document_type",
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
        write!(f, "FeishuDocReadError({label})")
    }
}

impl fmt::Display for FeishuDocReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidSourceRef => "invalid feishu doc source reference",
            Self::UnsupportedDocumentType => "unsupported feishu document type",
            Self::Unauthorized => "feishu doc token unauthorized",
            Self::Forbidden => "feishu doc permission denied",
            Self::NotFound => "feishu doc resource not found",
            Self::UpstreamClient => "feishu doc upstream rejected request",
            Self::UpstreamTransient => "feishu doc upstream transient failure",
            Self::Transport => "feishu doc transport failed",
            Self::ApiFailure => "feishu doc api failure",
            Self::InvalidJson => "feishu doc invalid json",
            Self::InvalidRequest => "feishu doc invalid request",
            Self::OversizedResponse => "feishu doc response too large",
        };
        f.write_str(message)
    }
}

impl std::error::Error for FeishuDocReadError {}

impl From<HttpClientFailure> for FeishuDocReadError {
    fn from(value: HttpClientFailure) -> Self {
        match value {
            HttpClientFailure::Transport => Self::Transport,
            HttpClientFailure::OversizedResponse { .. } => Self::OversizedResponse,
        }
    }
}

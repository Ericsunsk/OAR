use std::fmt;

use crate::oauth::HttpClientFailure;

#[derive(Clone, PartialEq, Eq)]
pub enum FeishuCalendarReadError {
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

impl fmt::Debug for FeishuCalendarReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidSourceRef => "FeishuCalendarReadError(invalid_source_ref)",
            Self::Unauthorized => "FeishuCalendarReadError(unauthorized)",
            Self::Forbidden => "FeishuCalendarReadError(forbidden)",
            Self::NotFound => "FeishuCalendarReadError(not_found)",
            Self::UpstreamClient => "FeishuCalendarReadError(upstream_client)",
            Self::UpstreamTransient => "FeishuCalendarReadError(upstream_transient)",
            Self::Transport => "FeishuCalendarReadError(transport)",
            Self::ApiFailure => "FeishuCalendarReadError(api_failure)",
            Self::InvalidJson => "FeishuCalendarReadError(invalid_json)",
            Self::InvalidRequest => "FeishuCalendarReadError(invalid_request)",
            Self::OversizedResponse => "FeishuCalendarReadError(oversized_response)",
        })
    }
}

impl fmt::Display for FeishuCalendarReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidSourceRef => "invalid calendar source reference",
            Self::Unauthorized => "feishu calendar token unauthorized",
            Self::Forbidden => "feishu calendar permission denied",
            Self::NotFound => "feishu calendar resource not found",
            Self::UpstreamClient => "feishu calendar upstream rejected request",
            Self::UpstreamTransient => "feishu calendar upstream transient failure",
            Self::Transport => "feishu calendar transport failed",
            Self::ApiFailure => "feishu calendar api failure",
            Self::InvalidJson => "feishu calendar invalid json",
            Self::InvalidRequest => "feishu calendar invalid request",
            Self::OversizedResponse => "feishu calendar response too large",
        })
    }
}

impl std::error::Error for FeishuCalendarReadError {}

impl From<HttpClientFailure> for FeishuCalendarReadError {
    fn from(value: HttpClientFailure) -> Self {
        match value {
            HttpClientFailure::Transport => Self::Transport,
            HttpClientFailure::OversizedResponse { .. } => Self::OversizedResponse,
        }
    }
}

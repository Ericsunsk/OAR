use std::fmt;

use crate::oauth::http::HttpClientFailure;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeishuOAuthLoginConfigError {
    InvalidOpenApi(FeishuOAuthLoginConfigInvalidField),
    EmptyAuthorizeBaseUrl,
    EmptyClientId,
    EmptyClientSecret,
    EmptyRedirectUri,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeishuOAuthLoginConfigInvalidField {
    OpenApi,
}

impl fmt::Display for FeishuOAuthLoginConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOpenApi(_) => write!(f, "feishu oauth login open api config is invalid"),
            Self::EmptyAuthorizeBaseUrl => write!(f, "feishu authorize base url is required"),
            Self::EmptyClientId => write!(f, "feishu client id is required"),
            Self::EmptyClientSecret => write!(f, "feishu client secret is required"),
            Self::EmptyRedirectUri => write!(f, "feishu redirect uri is required"),
        }
    }
}

impl std::error::Error for FeishuOAuthLoginConfigError {}

#[derive(Clone, PartialEq, Eq)]
pub enum FeishuOAuthLoginError {
    Transport,
    OversizedResponse { max_response_bytes: usize },
    InvalidTokenResponse,
    InvalidUserInfoResponse,
    TokenRejected { safe_error: String },
    UserInfoRejected { safe_error: String },
}

impl fmt::Debug for FeishuOAuthLoginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport => write!(f, "FeishuOAuthLoginError(transport)"),
            Self::OversizedResponse { max_response_bytes } => write!(
                f,
                "FeishuOAuthLoginError(oversized_response max={}B)",
                max_response_bytes
            ),
            Self::InvalidTokenResponse => {
                write!(f, "FeishuOAuthLoginError(invalid_token_response)")
            }
            Self::InvalidUserInfoResponse => {
                write!(f, "FeishuOAuthLoginError(invalid_user_info_response)")
            }
            Self::TokenRejected { safe_error } => f
                .debug_struct("FeishuOAuthLoginError(TokenRejected)")
                .field("safe_error", safe_error)
                .finish(),
            Self::UserInfoRejected { safe_error } => f
                .debug_struct("FeishuOAuthLoginError(UserInfoRejected)")
                .field("safe_error", safe_error)
                .finish(),
        }
    }
}

impl fmt::Display for FeishuOAuthLoginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport => write!(f, "feishu oauth login transport failed"),
            Self::OversizedResponse { max_response_bytes } => write!(
                f,
                "feishu oauth login response exceeded {} bytes",
                max_response_bytes
            ),
            Self::InvalidTokenResponse => write!(f, "feishu oauth token response is invalid"),
            Self::InvalidUserInfoResponse => {
                write!(f, "feishu oauth user info response is invalid")
            }
            Self::TokenRejected { safe_error } | Self::UserInfoRejected { safe_error } => {
                write!(f, "{safe_error}")
            }
        }
    }
}

impl std::error::Error for FeishuOAuthLoginError {}

pub(super) fn map_http_failure(error: HttpClientFailure) -> FeishuOAuthLoginError {
    match error {
        HttpClientFailure::Transport => FeishuOAuthLoginError::Transport,
        HttpClientFailure::OversizedResponse { max_response_bytes } => {
            FeishuOAuthLoginError::OversizedResponse { max_response_bytes }
        }
    }
}

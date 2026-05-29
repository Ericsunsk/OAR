use serde::Deserialize;

use crate::oauth::http::HttpResponse;
use crate::redaction::SecretString;

use super::error::FeishuOAuthLoginError;
use super::types::{FeishuOAuthLoginToken, FeishuOAuthLoginUser};

pub(super) fn parse_token_response(
    response: HttpResponse,
) -> Result<FeishuOAuthLoginToken, FeishuOAuthLoginError> {
    if response.status >= 500 {
        return Err(FeishuOAuthLoginError::TokenRejected {
            safe_error: "feishu oauth token temporarily unavailable".to_string(),
        });
    }

    let parsed: FeishuTokenResponse = serde_json::from_str(&response.body)
        .map_err(|_| FeishuOAuthLoginError::InvalidTokenResponse)?;

    match parsed.code {
        Some(0) => {
            let access_token = parsed
                .access_token
                .filter(|value| !value.trim().is_empty())
                .ok_or(FeishuOAuthLoginError::InvalidTokenResponse)?;
            let expires_in_seconds = parsed
                .expires_in
                .ok_or(FeishuOAuthLoginError::InvalidTokenResponse)?;
            Ok(FeishuOAuthLoginToken {
                access_token: SecretString::new(access_token),
                refresh_token: parsed
                    .refresh_token
                    .filter(|value| !value.trim().is_empty())
                    .map(SecretString::new),
                expires_in_seconds,
                refresh_token_expires_in_seconds: parsed.refresh_token_expires_in,
                token_type: parsed.token_type,
                scope: parsed.scope,
            })
        }
        Some(code) => Err(FeishuOAuthLoginError::TokenRejected {
            safe_error: safe_token_error(code).to_string(),
        }),
        None => Err(FeishuOAuthLoginError::InvalidTokenResponse),
    }
}

pub(super) fn parse_user_info_response(
    response: HttpResponse,
) -> Result<FeishuOAuthLoginUser, FeishuOAuthLoginError> {
    if response.status >= 500 {
        return Err(FeishuOAuthLoginError::UserInfoRejected {
            safe_error: "feishu user info temporarily unavailable".to_string(),
        });
    }

    let parsed: FeishuUserInfoResponse = serde_json::from_str(&response.body)
        .map_err(|_| FeishuOAuthLoginError::InvalidUserInfoResponse)?;

    match parsed.code {
        Some(0) => {
            let data = parsed
                .data
                .ok_or(FeishuOAuthLoginError::InvalidUserInfoResponse)?;
            let open_id = data
                .open_id
                .filter(|value| !value.trim().is_empty())
                .ok_or(FeishuOAuthLoginError::InvalidUserInfoResponse)?;
            let display_name = data
                .name
                .or(data.en_name)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| open_id.clone());
            Ok(FeishuOAuthLoginUser {
                open_id,
                union_id: data.union_id.filter(|value| !value.trim().is_empty()),
                tenant_key: data.tenant_key.filter(|value| !value.trim().is_empty()),
                display_name,
            })
        }
        Some(code) => Err(FeishuOAuthLoginError::UserInfoRejected {
            safe_error: safe_user_info_error(code).to_string(),
        }),
        None => Err(FeishuOAuthLoginError::InvalidUserInfoResponse),
    }
}

fn safe_token_error(code: i64) -> &'static str {
    match code {
        20003 | 20004 => "feishu authorization code is invalid or expired",
        20010 | 20069 => "feishu app is not available to this user",
        20067 | 20068 => "feishu oauth scope is invalid",
        _ => "feishu oauth token request was rejected",
    }
}

fn safe_user_info_error(_code: i64) -> &'static str {
    "feishu user info request was rejected"
}

#[derive(Deserialize)]
struct FeishuTokenResponse {
    code: Option<i64>,
    access_token: Option<String>,
    expires_in: Option<u64>,
    refresh_token: Option<String>,
    refresh_token_expires_in: Option<u64>,
    token_type: Option<String>,
    scope: Option<String>,
}

#[derive(Deserialize)]
struct FeishuUserInfoResponse {
    code: Option<i64>,
    data: Option<FeishuUserInfoData>,
}

#[derive(Deserialize)]
struct FeishuUserInfoData {
    name: Option<String>,
    en_name: Option<String>,
    open_id: Option<String>,
    union_id: Option<String>,
    tenant_key: Option<String>,
}

use serde_json::{json, Map};

use crate::http_headers::{bearer_json_headers_from_raw_token, json_headers};
use crate::oauth::http::HttpRequest;

use super::config::FeishuOAuthLoginConfig;

const TOKEN_PATH: &str = "/open-apis/authen/v2/oauth/token";
const USER_INFO_PATH: &str = "/open-apis/authen/v1/user_info";

pub(super) fn build_token_request(config: &FeishuOAuthLoginConfig, code: &str) -> HttpRequest {
    let mut body = Map::new();
    body.insert("grant_type".to_string(), json!("authorization_code"));
    body.insert("client_id".to_string(), json!(config.client_id));
    body.insert(
        "client_secret".to_string(),
        json!(config.client_secret.expose_secret()),
    );
    body.insert("code".to_string(), json!(code));
    body.insert("redirect_uri".to_string(), json!(config.redirect_uri));
    if let Some(scope) = &config.scope {
        body.insert("scope".to_string(), json!(scope));
    }

    HttpRequest {
        method: "POST".to_string(),
        url: format!(
            "{}{}",
            config.open_api.base_url.trim_end_matches('/'),
            TOKEN_PATH
        ),
        headers: json_headers(),
        body: json!(body),
        max_response_bytes: config.open_api.max_response_bytes,
    }
}

pub(super) fn build_user_info_request(
    config: &FeishuOAuthLoginConfig,
    access_token: &str,
) -> HttpRequest {
    HttpRequest {
        method: "GET".to_string(),
        url: format!(
            "{}{}",
            config.open_api.base_url.trim_end_matches('/'),
            USER_INFO_PATH
        ),
        headers: bearer_json_headers_from_raw_token(access_token),
        body: json!({}),
        max_response_bytes: config.open_api.max_response_bytes,
    }
}

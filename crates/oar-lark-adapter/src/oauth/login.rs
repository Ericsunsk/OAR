use std::fmt;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Map};

use crate::config::FeishuOpenApiConfig;
use crate::redaction::SecretString;
use crate::url_encoding::encode_query;

use super::http::{AsyncHttpClient, HttpClient, HttpRequest, HttpResponse};

const AUTHORIZE_PATH: &str = "/open-apis/authen/v1/authorize";
const TOKEN_PATH: &str = "/open-apis/authen/v2/oauth/token";
const USER_INFO_PATH: &str = "/open-apis/authen/v1/user_info";
const OAR_USER_AGENT: &str = concat!("oar-lark-adapter/", env!("CARGO_PKG_VERSION"));

#[derive(Clone)]
pub struct FeishuOAuthLoginConfig {
    pub open_api: FeishuOpenApiConfig,
    pub authorize_base_url: String,
    pub client_id: String,
    pub client_secret: SecretString,
    pub redirect_uri: String,
    pub scope: Option<String>,
}

impl fmt::Debug for FeishuOAuthLoginConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuOAuthLoginConfig")
            .field("open_api", &self.open_api)
            .field("authorize_base_url", &self.authorize_base_url)
            .field("client_id", &self.client_id)
            .field("client_secret", &"[REDACTED]")
            .field("redirect_uri", &self.redirect_uri)
            .field("scope", &self.scope)
            .finish()
    }
}

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

impl FeishuOAuthLoginConfig {
    pub fn new(
        open_api: FeishuOpenApiConfig,
        authorize_base_url: impl Into<String>,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        redirect_uri: impl Into<String>,
        scope: Option<String>,
    ) -> Result<Self, FeishuOAuthLoginConfigError> {
        open_api.validate().map_err(|_| {
            FeishuOAuthLoginConfigError::InvalidOpenApi(FeishuOAuthLoginConfigInvalidField::OpenApi)
        })?;
        let authorize_base_url = required_string(
            authorize_base_url.into(),
            FeishuOAuthLoginConfigError::EmptyAuthorizeBaseUrl,
        )?;
        let client_id =
            required_string(client_id.into(), FeishuOAuthLoginConfigError::EmptyClientId)?;
        let client_secret = required_string(
            client_secret.into(),
            FeishuOAuthLoginConfigError::EmptyClientSecret,
        )?;
        let redirect_uri = required_string(
            redirect_uri.into(),
            FeishuOAuthLoginConfigError::EmptyRedirectUri,
        )?;
        Ok(Self {
            open_api,
            authorize_base_url,
            client_id,
            client_secret: SecretString::new(client_secret),
            redirect_uri,
            scope: scope.and_then(|value| {
                let trimmed = value.trim();
                (!trimmed.is_empty()).then(|| trimmed.to_string())
            }),
        })
    }

    pub fn authorization_url(&self, state: &str) -> String {
        let mut pairs = vec![
            ("client_id", self.client_id.as_str()),
            ("response_type", "code"),
            ("redirect_uri", self.redirect_uri.as_str()),
            ("state", state),
        ];
        if let Some(scope) = &self.scope {
            pairs.push(("scope", scope.as_str()));
        }
        let query = encode_query(pairs);
        format!(
            "{}{}?{}",
            self.authorize_base_url.trim_end_matches('/'),
            AUTHORIZE_PATH,
            query
        )
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct FeishuOAuthLoginToken {
    pub access_token: SecretString,
    pub refresh_token: Option<SecretString>,
    pub expires_in_seconds: u64,
    pub refresh_token_expires_in_seconds: Option<u64>,
    pub token_type: Option<String>,
    pub scope: Option<String>,
}

impl fmt::Debug for FeishuOAuthLoginToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuOAuthLoginToken")
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &"[REDACTED]")
            .field("expires_in_seconds", &self.expires_in_seconds)
            .field(
                "refresh_token_expires_in_seconds",
                &self.refresh_token_expires_in_seconds,
            )
            .field("token_type", &self.token_type)
            .field("scope", &self.scope)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeishuOAuthLoginUser {
    pub open_id: String,
    pub union_id: Option<String>,
    pub tenant_key: Option<String>,
    pub display_name: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct FeishuOAuthLogin {
    pub token: FeishuOAuthLoginToken,
    pub user: FeishuOAuthLoginUser,
}

impl fmt::Debug for FeishuOAuthLogin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuOAuthLogin")
            .field("token", &self.token)
            .field("user", &self.user)
            .finish()
    }
}

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

pub struct FeishuOAuthLoginClient<H> {
    config: FeishuOAuthLoginConfig,
    http_client: H,
}

impl<H> FeishuOAuthLoginClient<H> {
    pub fn new(config: FeishuOAuthLoginConfig, http_client: H) -> Self {
        Self {
            config,
            http_client,
        }
    }

    pub fn config(&self) -> &FeishuOAuthLoginConfig {
        &self.config
    }
}

impl<H> fmt::Debug for FeishuOAuthLoginClient<H> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuOAuthLoginClient")
            .field("config", &self.config)
            .field("http_client", &"[REDACTED]")
            .finish()
    }
}

impl<H> FeishuOAuthLoginClient<H>
where
    H: HttpClient,
{
    pub fn exchange_code(&mut self, code: &str) -> Result<FeishuOAuthLogin, FeishuOAuthLoginError> {
        let token_response = self
            .http_client
            .send_json(build_token_request(&self.config, code))
            .map_err(map_http_failure)?;
        let token = parse_token_response(token_response)?;
        let user_response = self
            .http_client
            .send_json(build_user_info_request(
                &self.config,
                token.access_token.expose_secret(),
            ))
            .map_err(map_http_failure)?;
        let user = parse_user_info_response(user_response)?;
        Ok(FeishuOAuthLogin { token, user })
    }
}

#[async_trait]
pub trait AsyncFeishuOAuthLogin {
    async fn exchange_code(
        &mut self,
        code: &str,
    ) -> Result<FeishuOAuthLogin, FeishuOAuthLoginError>;
}

#[async_trait]
impl<H> AsyncFeishuOAuthLogin for FeishuOAuthLoginClient<H>
where
    H: AsyncHttpClient + Send,
{
    async fn exchange_code(
        &mut self,
        code: &str,
    ) -> Result<FeishuOAuthLogin, FeishuOAuthLoginError> {
        let token_response = self
            .http_client
            .send_json(build_token_request(&self.config, code))
            .await
            .map_err(map_http_failure)?;
        let token = parse_token_response(token_response)?;
        let user_response = self
            .http_client
            .send_json(build_user_info_request(
                &self.config,
                token.access_token.expose_secret(),
            ))
            .await
            .map_err(map_http_failure)?;
        let user = parse_user_info_response(user_response)?;
        Ok(FeishuOAuthLogin { token, user })
    }
}

pub fn build_token_request(config: &FeishuOAuthLoginConfig, code: &str) -> HttpRequest {
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
        headers: vec![
            (
                "Content-Type".to_string(),
                "application/json; charset=utf-8".to_string(),
            ),
            ("User-Agent".to_string(), OAR_USER_AGENT.to_string()),
        ],
        body: json!(body),
        max_response_bytes: config.open_api.max_response_bytes,
    }
}

pub fn build_user_info_request(config: &FeishuOAuthLoginConfig, access_token: &str) -> HttpRequest {
    HttpRequest {
        method: "GET".to_string(),
        url: format!(
            "{}{}",
            config.open_api.base_url.trim_end_matches('/'),
            USER_INFO_PATH
        ),
        headers: vec![
            (
                "Authorization".to_string(),
                format!("Bearer {access_token}"),
            ),
            (
                "Content-Type".to_string(),
                "application/json; charset=utf-8".to_string(),
            ),
            ("User-Agent".to_string(), OAR_USER_AGENT.to_string()),
        ],
        body: json!({}),
        max_response_bytes: config.open_api.max_response_bytes,
    }
}

fn parse_token_response(
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

fn parse_user_info_response(
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

fn map_http_failure(error: super::http::HttpClientFailure) -> FeishuOAuthLoginError {
    match error {
        super::http::HttpClientFailure::Transport => FeishuOAuthLoginError::Transport,
        super::http::HttpClientFailure::OversizedResponse { max_response_bytes } => {
            FeishuOAuthLoginError::OversizedResponse { max_response_bytes }
        }
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

fn required_string(
    value: String,
    error: FeishuOAuthLoginConfigError,
) -> Result<String, FeishuOAuthLoginConfigError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(error);
    }
    Ok(trimmed.to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FakeHttpClient {
        responses: Vec<Result<HttpResponse, super::super::http::HttpClientFailure>>,
        requests: Vec<HttpRequest>,
    }

    impl HttpClient for FakeHttpClient {
        fn post_json(
            &mut self,
            request: HttpRequest,
        ) -> Result<HttpResponse, super::super::http::HttpClientFailure> {
            self.send_json(request)
        }

        fn send_json(
            &mut self,
            request: HttpRequest,
        ) -> Result<HttpResponse, super::super::http::HttpClientFailure> {
            self.requests.push(request);
            self.responses.remove(0)
        }
    }

    fn config() -> FeishuOAuthLoginConfig {
        FeishuOAuthLoginConfig::new(
            FeishuOpenApiConfig::default(),
            "https://open.feishu.cn",
            "cli_test",
            "secret-value",
            "https://oar.example.test/auth/feishu/callback",
            Some("auth:user.id:read offline_access".to_string()),
        )
        .expect("config")
    }

    #[test]
    fn authorization_url_uses_latest_authorize_endpoint_and_encodes_query() {
        let url = config().authorization_url("state with space");

        assert_eq!(
            url,
            concat!(
                "https://open.feishu.cn/open-apis/authen/v1/authorize?",
                "client_id=cli_test",
                "&response_type=code",
                "&redirect_uri=https%3A%2F%2Foar.example.test%2Fauth%2Ffeishu%2Fcallback",
                "&state=state%20with%20space",
                "&scope=auth%3Auser.id%3Aread%20offline_access"
            )
        );
    }

    #[test]
    fn exchange_code_calls_token_then_user_info_without_debug_leaking_secrets() {
        let mut client = FeishuOAuthLoginClient::new(
            config(),
            FakeHttpClient {
                responses: vec![
                    Ok(HttpResponse::new(
                        200,
                        r#"{
                          "code": 0,
                          "access_token": "access-secret",
                          "refresh_token": "refresh-secret",
                          "expires_in": 7200,
                          "refresh_token_expires_in": 2592000,
                          "token_type": "Bearer",
                          "scope": "offline_access"
                        }"#,
                    )),
                    Ok(HttpResponse::new(
                        200,
                        r#"{
                          "code": 0,
                          "data": {
                            "name": "陈敏",
                            "open_id": "ou_123",
                            "union_id": "on_123",
                            "tenant_key": "tenant_123"
                          }
                        }"#,
                    )),
                ],
                requests: vec![],
            },
        );

        let login = client.exchange_code("code-secret").expect("login");
        assert_eq!(login.user.open_id, "ou_123");
        assert_eq!(login.user.display_name, "陈敏");
        assert_eq!(login.user.tenant_key.as_deref(), Some("tenant_123"));

        let debug = format!("{login:?}");
        assert!(!debug.contains("access-secret"));
        assert!(!debug.contains("refresh-secret"));
        assert!(!debug.contains("code-secret"));
    }
}

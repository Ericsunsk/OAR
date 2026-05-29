use std::fmt;

use async_trait::async_trait;

use crate::oauth::http::{AsyncHttpClient, HttpClient};

use super::config::FeishuOAuthLoginConfig;
use super::error::{map_http_failure, FeishuOAuthLoginError};
use super::request_builder::{build_token_request, build_user_info_request};
use super::response_parser::{parse_token_response, parse_user_info_response};
use super::types::FeishuOAuthLogin;

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

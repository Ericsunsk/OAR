use async_trait::async_trait;
use serde_json::json;

use crate::config::FeishuOpenApiConfig;
use crate::http_headers::bearer_accept_headers;
use crate::oauth::{AsyncHttpClient, HttpClient, HttpRequest};
use crate::redaction::SecretString;
use crate::url_encoding::percent_encode;

use super::error::FeishuMinutesReadError;
use super::response_parser::map_status_or_parse_minute;
use super::source_ref::{parse_minutes_source_ref, valid_minute_token};
use super::types::{FeishuMinuteReadRequest, MinuteReadSummary};

const MINUTES_GET_PATH_PREFIX: &str = "/open-apis/minutes/v1/minutes";

#[derive(Debug, Clone)]
pub struct FeishuMinutesReadClient<H> {
    config: FeishuOpenApiConfig,
    http_client: H,
}

impl<H> FeishuMinutesReadClient<H> {
    pub fn new(config: FeishuOpenApiConfig, http_client: H) -> Self {
        Self {
            config,
            http_client,
        }
    }

    pub fn http_client(&self) -> &H {
        &self.http_client
    }
}

impl<H> FeishuMinutesReadClient<H>
where
    H: HttpClient,
{
    pub fn get_minute_summary(
        &mut self,
        request: FeishuMinuteReadRequest,
    ) -> Result<MinuteReadSummary, FeishuMinutesReadError> {
        let source_ref = parse_minutes_source_ref(&request.source_ref)?;
        let raw = self
            .http_client
            .send_json(build_get_minute_request(
                &self.config,
                &request.user_access_token,
                &source_ref.minute_token,
            )?)
            .map_err(FeishuMinutesReadError::from)?;
        map_status_or_parse_minute(raw.status, &raw.body)
    }
}

#[async_trait]
pub trait AsyncFeishuMinutesRead {
    async fn get_minute_summary(
        &mut self,
        request: FeishuMinuteReadRequest,
    ) -> Result<MinuteReadSummary, FeishuMinutesReadError>;
}

#[async_trait]
impl<H> AsyncFeishuMinutesRead for FeishuMinutesReadClient<H>
where
    H: AsyncHttpClient + Send,
{
    async fn get_minute_summary(
        &mut self,
        request: FeishuMinuteReadRequest,
    ) -> Result<MinuteReadSummary, FeishuMinutesReadError> {
        let source_ref = parse_minutes_source_ref(&request.source_ref)?;
        let raw = self
            .http_client
            .send_json(build_get_minute_request(
                &self.config,
                &request.user_access_token,
                &source_ref.minute_token,
            )?)
            .await
            .map_err(FeishuMinutesReadError::from)?;
        map_status_or_parse_minute(raw.status, &raw.body)
    }
}

pub fn build_get_minute_request(
    config: &FeishuOpenApiConfig,
    user_access_token: &SecretString,
    minute_token: &str,
) -> Result<HttpRequest, FeishuMinutesReadError> {
    validate_token(minute_token)?;
    Ok(HttpRequest {
        method: "GET".to_string(),
        url: format!(
            "{}/{}/{}",
            config.base_url.trim_end_matches('/'),
            MINUTES_GET_PATH_PREFIX.trim_start_matches('/'),
            percent_encode(minute_token.trim())
        ),
        headers: bearer_accept_headers(user_access_token),
        body: json!({}),
        max_response_bytes: config.max_response_bytes,
    })
}

fn validate_token(token: &str) -> Result<(), FeishuMinutesReadError> {
    if valid_minute_token(token.trim()) {
        Ok(())
    } else {
        Err(FeishuMinutesReadError::InvalidRequest)
    }
}

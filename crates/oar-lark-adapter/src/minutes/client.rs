use async_trait::async_trait;
use serde_json::{json, Map, Value};

use crate::config::FeishuOpenApiConfig;
use crate::http_headers::{bearer_accept_headers, bearer_json_headers};
use crate::oauth::{AsyncHttpClient, HttpClient, HttpRequest};
use crate::redaction::SecretString;
use crate::url_encoding::{encode_query, percent_encode};

use super::error::FeishuMinutesReadError;
use super::response_parser::{map_status_or_parse_minute, map_status_or_parse_minute_search};
use super::source_ref::{parse_minutes_source_ref, valid_minute_token};
use super::types::{
    FeishuMinuteReadRequest, FeishuMinuteSearchRequest, MinuteReadSummary, MinuteSearchPage,
};

const MINUTES_GET_PATH_PREFIX: &str = "/open-apis/minutes/v1/minutes";
const MINUTES_SEARCH_PATH: &str = "/open-apis/minutes/v1/minutes/search";

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

    pub fn search_minute_summaries(
        &mut self,
        request: FeishuMinuteSearchRequest,
    ) -> Result<MinuteSearchPage, FeishuMinutesReadError> {
        let raw = self
            .http_client
            .send_json(build_search_minutes_request(&self.config, request)?)
            .map_err(FeishuMinutesReadError::from)?;
        map_status_or_parse_minute_search(raw.status, &raw.body)
    }
}

#[async_trait]
pub trait AsyncFeishuMinutesRead {
    async fn get_minute_summary(
        &mut self,
        request: FeishuMinuteReadRequest,
    ) -> Result<MinuteReadSummary, FeishuMinutesReadError>;

    async fn search_minute_summaries(
        &mut self,
        request: FeishuMinuteSearchRequest,
    ) -> Result<MinuteSearchPage, FeishuMinutesReadError>;
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

    async fn search_minute_summaries(
        &mut self,
        request: FeishuMinuteSearchRequest,
    ) -> Result<MinuteSearchPage, FeishuMinutesReadError> {
        let raw = self
            .http_client
            .send_json(build_search_minutes_request(&self.config, request)?)
            .await
            .map_err(FeishuMinutesReadError::from)?;
        map_status_or_parse_minute_search(raw.status, &raw.body)
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

pub fn build_search_minutes_request(
    config: &FeishuOpenApiConfig,
    request: FeishuMinuteSearchRequest,
) -> Result<HttpRequest, FeishuMinutesReadError> {
    let mut query_parts = vec![(
        "page_size",
        request.page_size.unwrap_or(15).clamp(1, 30).to_string(),
    )];
    if let Some(page_token) = non_empty_trimmed(request.page_token) {
        validate_page_token(&page_token)?;
        query_parts.push(("page_token", page_token));
    }

    let mut body = Map::new();
    if let Some(query) = non_empty_trimmed(request.query) {
        validate_query(&query)?;
        body.insert("query".to_string(), Value::String(query));
    }

    let mut filter = Map::new();
    insert_user_id_filter(&mut filter, "owner_ids", request.owner_ids)?;
    insert_user_id_filter(&mut filter, "participant_ids", request.participant_ids)?;
    if !filter.is_empty() {
        body.insert("filter".to_string(), Value::Object(filter));
    }
    if body.is_empty() {
        return Err(FeishuMinutesReadError::InvalidRequest);
    }

    Ok(HttpRequest {
        method: "POST".to_string(),
        url: format!(
            "{}/{}?{}",
            config.base_url.trim_end_matches('/'),
            MINUTES_SEARCH_PATH.trim_start_matches('/'),
            encode_query(query_parts)
        ),
        headers: bearer_json_headers(&request.user_access_token),
        body: Value::Object(body),
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

fn insert_user_id_filter(
    filter: &mut Map<String, Value>,
    field: &str,
    ids: Vec<String>,
) -> Result<(), FeishuMinutesReadError> {
    let ids = ids
        .into_iter()
        .filter_map(|id| non_empty_trimmed(Some(id)))
        .map(|id| {
            validate_user_id(&id)?;
            Ok(Value::String(id))
        })
        .collect::<Result<Vec<_>, FeishuMinutesReadError>>()?;
    if ids.len() > 100 {
        return Err(FeishuMinutesReadError::InvalidRequest);
    }
    if !ids.is_empty() {
        filter.insert(field.to_string(), Value::Array(ids));
    }
    Ok(())
}

fn validate_query(value: &str) -> Result<(), FeishuMinutesReadError> {
    if value.chars().count() <= 50 && value.chars().all(|ch| !ch.is_control()) {
        Ok(())
    } else {
        Err(FeishuMinutesReadError::InvalidRequest)
    }
}

fn validate_user_id(value: &str) -> Result<(), FeishuMinutesReadError> {
    if !value.is_empty()
        && value.len() <= 128
        && value
            .chars()
            .all(|ch| !ch.is_whitespace() && !ch.is_control() && !matches!(ch, '/' | '?' | '#'))
    {
        Ok(())
    } else {
        Err(FeishuMinutesReadError::InvalidRequest)
    }
}

fn validate_page_token(value: &str) -> Result<(), FeishuMinutesReadError> {
    if !value.is_empty()
        && value.len() <= 512
        && value
            .chars()
            .all(|ch| !ch.is_whitespace() && !ch.is_control())
    {
        Ok(())
    } else {
        Err(FeishuMinutesReadError::InvalidRequest)
    }
}

fn non_empty_trimmed(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

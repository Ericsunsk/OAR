use async_trait::async_trait;
use serde_json::json;

use crate::config::FeishuOpenApiConfig;
use crate::oauth::{AsyncHttpClient, HttpClient, HttpRequest};
use crate::redaction::SecretString;

use super::error::FeishuCalendarReadError;
use super::types::{
    valid_calendar_user_id, valid_rfc3339ish_time, CalendarFreeBusyBatchRequest,
    CalendarFreeBusyPage, FeishuFreeBusyBatchResponse,
};

const FREE_BUSY_BATCH_PATH: &str = "/open-apis/calendar/v4/freebusy/batch";
const OAR_USER_AGENT: &str = concat!("oar-lark-adapter/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Clone)]
pub struct FeishuCalendarReadClient<H> {
    config: FeishuOpenApiConfig,
    http_client: H,
}

impl<H> FeishuCalendarReadClient<H> {
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

impl<H> FeishuCalendarReadClient<H>
where
    H: HttpClient,
{
    pub fn batch_free_busy(
        &mut self,
        request: CalendarFreeBusyBatchRequest,
    ) -> Result<CalendarFreeBusyPage, FeishuCalendarReadError> {
        let raw = self
            .http_client
            .send_json(build_free_busy_batch_request(&self.config, request)?)
            .map_err(FeishuCalendarReadError::from)?;
        map_status_or_parse_free_busy(raw.status, &raw.body)
    }
}

#[async_trait]
pub trait AsyncFeishuCalendarRead {
    async fn batch_free_busy(
        &mut self,
        request: CalendarFreeBusyBatchRequest,
    ) -> Result<CalendarFreeBusyPage, FeishuCalendarReadError>;
}

#[async_trait]
impl<H> AsyncFeishuCalendarRead for FeishuCalendarReadClient<H>
where
    H: AsyncHttpClient + Send,
{
    async fn batch_free_busy(
        &mut self,
        request: CalendarFreeBusyBatchRequest,
    ) -> Result<CalendarFreeBusyPage, FeishuCalendarReadError> {
        let raw = self
            .http_client
            .send_json(build_free_busy_batch_request(&self.config, request)?)
            .await
            .map_err(FeishuCalendarReadError::from)?;
        map_status_or_parse_free_busy(raw.status, &raw.body)
    }
}

pub fn build_free_busy_batch_request(
    config: &FeishuOpenApiConfig,
    request: CalendarFreeBusyBatchRequest,
) -> Result<HttpRequest, FeishuCalendarReadError> {
    validate_request(&request)?;
    let url = format!(
        "{}/{}?user_id_type={}",
        config.base_url.trim_end_matches('/'),
        FREE_BUSY_BATCH_PATH.trim_start_matches('/'),
        request.user_id_type.as_str()
    );
    let user_ids = request
        .user_ids
        .iter()
        .map(|user_id| user_id.trim().to_string())
        .collect::<Vec<_>>();

    Ok(HttpRequest {
        method: "POST".to_string(),
        url,
        headers: calendar_request_headers(&request.user_access_token),
        body: json!({
            "time_min": request.time_min.trim(),
            "time_max": request.time_max.trim(),
            "user_ids": user_ids,
            "include_external_calendar": request.include_external_calendar,
            "only_busy": request.only_busy,
            "need_rsvp_status": request.need_rsvp_status,
        }),
        max_response_bytes: config.max_response_bytes,
    })
}

fn validate_request(request: &CalendarFreeBusyBatchRequest) -> Result<(), FeishuCalendarReadError> {
    if request.user_ids.is_empty() || request.user_ids.len() > 10 {
        return Err(FeishuCalendarReadError::InvalidRequest);
    }
    if request
        .user_ids
        .iter()
        .any(|user_id| !valid_calendar_user_id(user_id.trim()))
    {
        return Err(FeishuCalendarReadError::InvalidRequest);
    }
    if !valid_rfc3339ish_time(&request.time_min) || !valid_rfc3339ish_time(&request.time_max) {
        return Err(FeishuCalendarReadError::InvalidRequest);
    }
    Ok(())
}

fn calendar_request_headers(user_access_token: &SecretString) -> Vec<(String, String)> {
    vec![
        (
            "Authorization".to_string(),
            format!("Bearer {}", user_access_token.expose_secret()),
        ),
        ("Accept".to_string(), "application/json".to_string()),
        (
            "Content-Type".to_string(),
            "application/json; charset=utf-8".to_string(),
        ),
        ("User-Agent".to_string(), OAR_USER_AGENT.to_string()),
    ]
}

fn map_status_or_parse_free_busy(
    status: u16,
    body: &str,
) -> Result<CalendarFreeBusyPage, FeishuCalendarReadError> {
    match status {
        200..=299 => {
            let parsed: FeishuFreeBusyBatchResponse =
                serde_json::from_str(body).map_err(|_| FeishuCalendarReadError::InvalidJson)?;
            if parsed.code != 0 {
                return Err(map_api_code(parsed.code));
            }
            let data = parsed.data.ok_or(FeishuCalendarReadError::InvalidJson)?;
            Ok(CalendarFreeBusyPage::from_feishu_data(data))
        }
        401 => Err(FeishuCalendarReadError::Unauthorized),
        403 => Err(FeishuCalendarReadError::Forbidden),
        404 => Err(FeishuCalendarReadError::NotFound),
        429 => Err(FeishuCalendarReadError::UpstreamTransient),
        400..=499 => Err(FeishuCalendarReadError::UpstreamClient),
        _ => Err(FeishuCalendarReadError::UpstreamTransient),
    }
}

fn map_api_code(code: i64) -> FeishuCalendarReadError {
    match code {
        401 | 99991663 | 99991664 => FeishuCalendarReadError::Unauthorized,
        403 | 190006 => FeishuCalendarReadError::Forbidden,
        404 | 190007 | 195100 => FeishuCalendarReadError::NotFound,
        190002 | 190014 | 198001..=198004 => FeishuCalendarReadError::UpstreamClient,
        190003 | 190004 | 190005 | 190010 => FeishuCalendarReadError::UpstreamTransient,
        _ => FeishuCalendarReadError::ApiFailure,
    }
}

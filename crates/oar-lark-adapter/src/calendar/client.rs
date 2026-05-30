use async_trait::async_trait;
use serde_json::json;

use crate::config::FeishuOpenApiConfig;
use crate::http_headers::{bearer_accept_headers, bearer_json_headers};
use crate::oauth::{AsyncHttpClient, HttpClient, HttpRequest};
use crate::url_encoding::{encode_query, percent_encode};

use super::error::FeishuCalendarReadError;
use super::types::{
    valid_calendar_id, valid_calendar_user_id, valid_rfc3339ish_time, CalendarEventInstancePage,
    CalendarEventInstanceViewRequest, CalendarFreeBusyBatchRequest, CalendarFreeBusyPage,
    CalendarPrimaryPage, CalendarPrimaryRequest, FeishuEventInstanceViewResponse,
    FeishuFreeBusyBatchResponse, FeishuPrimaryCalendarResponse,
};

const FREE_BUSY_BATCH_PATH: &str = "/open-apis/calendar/v4/freebusy/batch";
const PRIMARY_CALENDAR_PATH: &str = "/open-apis/calendar/v4/calendars/primary";
const EVENT_INSTANCE_VIEW_PATH_SUFFIX: &str = "events/instance_view";
const MAX_INSTANCE_VIEW_WINDOW_SECONDS: i64 = 40 * 24 * 60 * 60;

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

    pub fn primary_calendar(
        &mut self,
        request: CalendarPrimaryRequest,
    ) -> Result<CalendarPrimaryPage, FeishuCalendarReadError> {
        let raw = self
            .http_client
            .send_json(build_primary_calendar_request(&self.config, request)?)
            .map_err(FeishuCalendarReadError::from)?;
        map_status_or_parse_primary(raw.status, &raw.body)
    }

    pub fn event_instance_view(
        &mut self,
        request: CalendarEventInstanceViewRequest,
    ) -> Result<CalendarEventInstancePage, FeishuCalendarReadError> {
        let raw = self
            .http_client
            .send_json(build_event_instance_view_request(&self.config, request)?)
            .map_err(FeishuCalendarReadError::from)?;
        map_status_or_parse_instance_view(raw.status, &raw.body)
    }
}

#[async_trait]
pub trait AsyncFeishuCalendarRead {
    async fn batch_free_busy(
        &mut self,
        request: CalendarFreeBusyBatchRequest,
    ) -> Result<CalendarFreeBusyPage, FeishuCalendarReadError>;

    async fn primary_calendar(
        &mut self,
        request: CalendarPrimaryRequest,
    ) -> Result<CalendarPrimaryPage, FeishuCalendarReadError>;

    async fn event_instance_view(
        &mut self,
        request: CalendarEventInstanceViewRequest,
    ) -> Result<CalendarEventInstancePage, FeishuCalendarReadError>;
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

    async fn primary_calendar(
        &mut self,
        request: CalendarPrimaryRequest,
    ) -> Result<CalendarPrimaryPage, FeishuCalendarReadError> {
        let raw = self
            .http_client
            .send_json(build_primary_calendar_request(&self.config, request)?)
            .await
            .map_err(FeishuCalendarReadError::from)?;
        map_status_or_parse_primary(raw.status, &raw.body)
    }

    async fn event_instance_view(
        &mut self,
        request: CalendarEventInstanceViewRequest,
    ) -> Result<CalendarEventInstancePage, FeishuCalendarReadError> {
        let raw = self
            .http_client
            .send_json(build_event_instance_view_request(&self.config, request)?)
            .await
            .map_err(FeishuCalendarReadError::from)?;
        map_status_or_parse_instance_view(raw.status, &raw.body)
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
        headers: bearer_json_headers(&request.user_access_token),
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

pub fn build_primary_calendar_request(
    config: &FeishuOpenApiConfig,
    request: CalendarPrimaryRequest,
) -> Result<HttpRequest, FeishuCalendarReadError> {
    Ok(HttpRequest {
        method: "POST".to_string(),
        url: format!(
            "{}/{}",
            config.base_url.trim_end_matches('/'),
            PRIMARY_CALENDAR_PATH.trim_start_matches('/')
        ),
        headers: bearer_json_headers(&request.user_access_token),
        body: json!({}),
        max_response_bytes: config.max_response_bytes,
    })
}

pub fn build_event_instance_view_request(
    config: &FeishuOpenApiConfig,
    request: CalendarEventInstanceViewRequest,
) -> Result<HttpRequest, FeishuCalendarReadError> {
    validate_event_instance_view_request(&request)?;
    let query_parts = vec![
        ("start_time", request.start_time.to_string()),
        ("end_time", request.end_time.to_string()),
    ];
    let query_string = encode_query(query_parts);
    let url = format!(
        "{}/open-apis/calendar/v4/calendars/{}/{}?{}",
        config.base_url.trim_end_matches('/'),
        percent_encode(request.calendar_id.trim()),
        EVENT_INSTANCE_VIEW_PATH_SUFFIX,
        query_string
    );

    Ok(HttpRequest {
        method: "GET".to_string(),
        url,
        headers: bearer_accept_headers(&request.user_access_token),
        body: json!({}),
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

fn validate_event_instance_view_request(
    request: &CalendarEventInstanceViewRequest,
) -> Result<(), FeishuCalendarReadError> {
    if !valid_calendar_id(&request.calendar_id) {
        return Err(FeishuCalendarReadError::InvalidRequest);
    }
    if request.start_time < 0
        || request.end_time <= request.start_time
        || request.end_time - request.start_time >= MAX_INSTANCE_VIEW_WINDOW_SECONDS
    {
        return Err(FeishuCalendarReadError::InvalidRequest);
    }
    Ok(())
}

fn map_status_or_parse_free_busy(
    status: u16,
    body: &str,
) -> Result<CalendarFreeBusyPage, FeishuCalendarReadError> {
    map_status_or_parse_calendar_response(status, body, |body| {
        let parsed: FeishuFreeBusyBatchResponse =
            serde_json::from_str(body).map_err(|_| FeishuCalendarReadError::InvalidJson)?;
        if parsed.code != 0 {
            return Err(map_api_code(parsed.code));
        }
        let data = parsed.data.ok_or(FeishuCalendarReadError::InvalidJson)?;
        Ok(CalendarFreeBusyPage::from_feishu_data(data))
    })
}

fn map_status_or_parse_primary(
    status: u16,
    body: &str,
) -> Result<CalendarPrimaryPage, FeishuCalendarReadError> {
    map_status_or_parse_calendar_response(status, body, |body| {
        let parsed: FeishuPrimaryCalendarResponse =
            serde_json::from_str(body).map_err(|_| FeishuCalendarReadError::InvalidJson)?;
        if parsed.code != 0 {
            return Err(map_api_code(parsed.code));
        }
        let data = parsed.data.ok_or(FeishuCalendarReadError::InvalidJson)?;
        CalendarPrimaryPage::from_feishu_data(data).ok_or(FeishuCalendarReadError::InvalidJson)
    })
}

fn map_status_or_parse_instance_view(
    status: u16,
    body: &str,
) -> Result<CalendarEventInstancePage, FeishuCalendarReadError> {
    map_status_or_parse_calendar_response(status, body, |body| {
        let parsed: FeishuEventInstanceViewResponse =
            serde_json::from_str(body).map_err(|_| FeishuCalendarReadError::InvalidJson)?;
        if parsed.code != 0 {
            return Err(map_api_code(parsed.code));
        }
        let data = parsed.data.ok_or(FeishuCalendarReadError::InvalidJson)?;
        Ok(CalendarEventInstancePage::from_feishu_data(data))
    })
}

fn map_status_or_parse_calendar_response<T>(
    status: u16,
    body: &str,
    parse_success: impl FnOnce(&str) -> Result<T, FeishuCalendarReadError>,
) -> Result<T, FeishuCalendarReadError> {
    match status {
        200..=299 => parse_success(body),
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

use super::error::FeishuCalendarReadError;
use super::feishu_types::{
    FeishuEventInstanceViewResponse, FeishuFreeBusyBatchResponse, FeishuPrimaryCalendarResponse,
};
use super::types::{CalendarEventInstancePage, CalendarFreeBusyPage, CalendarPrimaryPage};

pub(super) fn map_status_or_parse_free_busy(
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

pub(super) fn map_status_or_parse_primary(
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

pub(super) fn map_status_or_parse_instance_view(
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

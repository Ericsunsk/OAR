use serde::de::DeserializeOwned;

use super::error::FeishuOkrReadError;
use super::types::{
    FeishuOkrBatchGetResponse, FeishuOkrCycleListResponse, FeishuOkrCycleObjectivesListResponse,
    FeishuOkrObjectiveKeyResultsListResponse, FeishuOkrProgressListResponse,
};

pub(super) fn map_status_or_parse_batch_get(
    status: u16,
    body: &str,
) -> Result<FeishuOkrBatchGetResponse, FeishuOkrReadError> {
    map_status_or_parse_okr_response(status, body)
}

pub(super) fn map_status_or_parse_cycle_list(
    status: u16,
    body: &str,
) -> Result<FeishuOkrCycleListResponse, FeishuOkrReadError> {
    map_status_or_parse_okr_response(status, body)
}

pub(super) fn map_status_or_parse_cycle_objectives_list(
    status: u16,
    body: &str,
) -> Result<FeishuOkrCycleObjectivesListResponse, FeishuOkrReadError> {
    map_status_or_parse_okr_response(status, body)
}

pub(super) fn map_status_or_parse_objective_key_results_list(
    status: u16,
    body: &str,
) -> Result<FeishuOkrObjectiveKeyResultsListResponse, FeishuOkrReadError> {
    map_status_or_parse_okr_response(status, body)
}

pub(super) fn map_status_or_parse_progress_list(
    status: u16,
    body: &str,
) -> Result<FeishuOkrProgressListResponse, FeishuOkrReadError> {
    map_status_or_parse_okr_response(status, body)
}

fn map_status_or_parse_okr_response<T>(status: u16, body: &str) -> Result<T, FeishuOkrReadError>
where
    T: DeserializeOwned + OkrApiEnvelope,
{
    match status {
        200..=299 => {
            let parsed: T =
                serde_json::from_str(body).map_err(|_| FeishuOkrReadError::InvalidJson)?;
            if parsed.code() != 0 {
                return Err(map_api_code(parsed.code()));
            }
            Ok(parsed)
        }
        401 => Err(FeishuOkrReadError::Unauthorized),
        403 => Err(FeishuOkrReadError::Forbidden),
        400..=499 => Err(FeishuOkrReadError::UpstreamClient),
        _ => Err(FeishuOkrReadError::UpstreamTransient),
    }
}

trait OkrApiEnvelope {
    fn code(&self) -> i64;
}

impl OkrApiEnvelope for FeishuOkrBatchGetResponse {
    fn code(&self) -> i64 {
        self.code
    }
}

impl OkrApiEnvelope for FeishuOkrCycleListResponse {
    fn code(&self) -> i64 {
        self.code
    }
}

impl OkrApiEnvelope for FeishuOkrCycleObjectivesListResponse {
    fn code(&self) -> i64 {
        self.code
    }
}

impl OkrApiEnvelope for FeishuOkrObjectiveKeyResultsListResponse {
    fn code(&self) -> i64 {
        self.code
    }
}

impl OkrApiEnvelope for FeishuOkrProgressListResponse {
    fn code(&self) -> i64 {
        self.code
    }
}

fn map_api_code(code: i64) -> FeishuOkrReadError {
    match code {
        401 | 99991663 | 99991664 => FeishuOkrReadError::Unauthorized,
        403 | 1001002 => FeishuOkrReadError::Forbidden,
        400..=499 => FeishuOkrReadError::UpstreamClient,
        _ => FeishuOkrReadError::ApiFailure,
    }
}

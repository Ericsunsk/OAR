use super::error::FeishuOkrReadError;
use super::types::{FeishuOkrBatchGetRequest, FeishuOkrProgressListRequest};

const MAX_BATCH_GET_OKR_IDS: usize = 10;
const MAX_PAGE_SIZE: u32 = 100;
const MAX_PATH_ID_BYTES: usize = 256;
const MAX_PAGE_TOKEN_BYTES: usize = 512;
const MAX_LANG_BYTES: usize = 32;

pub(super) fn validate_batch_get_request(
    request: &FeishuOkrBatchGetRequest,
) -> Result<(), FeishuOkrReadError> {
    if request.okr_ids.is_empty() || request.okr_ids.len() > MAX_BATCH_GET_OKR_IDS {
        return Err(FeishuOkrReadError::InvalidRequest);
    }
    Ok(())
}

pub(super) fn validate_page_request(
    page_size: Option<u32>,
    page_token: Option<&str>,
    lang: Option<&str>,
) -> Result<(), FeishuOkrReadError> {
    if page_size
        .map(|page_size| page_size == 0 || page_size > MAX_PAGE_SIZE)
        .unwrap_or(false)
    {
        return Err(FeishuOkrReadError::InvalidRequest);
    }
    validate_optional_len(page_token, MAX_PAGE_TOKEN_BYTES)?;
    validate_optional_len(lang, MAX_LANG_BYTES)?;
    Ok(())
}

pub(super) fn validate_progress_list_request(
    request: &FeishuOkrProgressListRequest,
    default_page_size: u32,
) -> Result<(), FeishuOkrReadError> {
    validate_path_id(request.target.id())?;
    let page_size = request.page_size.unwrap_or(default_page_size);
    if page_size == 0 || page_size > MAX_PAGE_SIZE {
        return Err(FeishuOkrReadError::InvalidRequest);
    }
    validate_non_empty_optional_len(request.page_token.as_deref(), MAX_PAGE_TOKEN_BYTES)?;
    Ok(())
}

pub(super) fn validate_path_id(value: &str) -> Result<(), FeishuOkrReadError> {
    if value.trim().is_empty() || value.len() > MAX_PATH_ID_BYTES {
        return Err(FeishuOkrReadError::InvalidRequest);
    }
    Ok(())
}

fn validate_optional_len(value: Option<&str>, max_len: usize) -> Result<(), FeishuOkrReadError> {
    if value.map(|value| value.len() > max_len).unwrap_or(false) {
        return Err(FeishuOkrReadError::InvalidRequest);
    }
    Ok(())
}

fn validate_non_empty_optional_len(
    value: Option<&str>,
    max_len: usize,
) -> Result<(), FeishuOkrReadError> {
    if value
        .map(|value| value.trim().is_empty() || value.len() > max_len)
        .unwrap_or(false)
    {
        return Err(FeishuOkrReadError::InvalidRequest);
    }
    Ok(())
}

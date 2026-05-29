use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::config::FeishuOpenApiConfig;
use crate::oauth::{AsyncHttpClient, HttpClient, HttpRequest};

use super::error::FeishuOkrReadError;
use super::types::{
    FeishuOkrBatchGetRequest, FeishuOkrBatchGetResponse, FeishuOkrCycleListRequest,
    FeishuOkrCycleListResponse, FeishuOkrCycleObjectivesListRequest,
    FeishuOkrCycleObjectivesListResponse, FeishuOkrObjectiveKeyResultsListRequest,
    FeishuOkrObjectiveKeyResultsListResponse, FeishuOkrProgressListRequest,
    FeishuOkrProgressListResponse, FeishuOkrProgressListTarget,
};

const OKR_BATCH_GET_PATH: &str = "/open-apis/okr/v1/okrs/batch_get";
const OKR_CYCLES_PATH: &str = "/open-apis/okr/v2/cycles";
const OKR_OBJECTIVES_PATH: &str = "/open-apis/okr/v2/objectives";
const OKR_KEY_RESULTS_PATH: &str = "/open-apis/okr/v2/key_results";
const OAR_USER_AGENT: &str = concat!("oar-lark-adapter/", env!("CARGO_PKG_VERSION"));
const DEFAULT_PAGE_SIZE: u32 = 100;
const MAX_PAGE_SIZE: u32 = 100;
const MAX_PATH_ID_BYTES: usize = 256;
const MAX_PAGE_TOKEN_BYTES: usize = 512;
const MAX_LANG_BYTES: usize = 32;

#[derive(Debug, Clone)]
pub struct FeishuOkrReadClient<H> {
    config: FeishuOpenApiConfig,
    http_client: H,
}

impl<H> FeishuOkrReadClient<H> {
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

impl<H> FeishuOkrReadClient<H>
where
    H: HttpClient,
{
    pub fn batch_get_okrs(
        &mut self,
        request: FeishuOkrBatchGetRequest,
    ) -> Result<FeishuOkrBatchGetResponse, FeishuOkrReadError> {
        if request.okr_ids.is_empty() || request.okr_ids.len() > 10 {
            return Err(FeishuOkrReadError::InvalidRequest);
        }
        let raw = self
            .http_client
            .send_json(build_batch_get_okr_request(&self.config, request))
            .map_err(FeishuOkrReadError::from)?;
        map_status_or_parse_batch_get(raw.status, &raw.body)
    }

    pub fn list_cycles(
        &mut self,
        request: FeishuOkrCycleListRequest,
    ) -> Result<FeishuOkrCycleListResponse, FeishuOkrReadError> {
        validate_path_id(&request.user_id)?;
        validate_page_request(
            request.page_size,
            request.page_token.as_deref(),
            request.lang.as_deref(),
        )?;
        let raw = self
            .http_client
            .send_json(build_list_cycles_request(&self.config, request))
            .map_err(FeishuOkrReadError::from)?;
        map_status_or_parse_cycle_list(raw.status, &raw.body)
    }

    pub fn list_cycle_objectives(
        &mut self,
        request: FeishuOkrCycleObjectivesListRequest,
    ) -> Result<FeishuOkrCycleObjectivesListResponse, FeishuOkrReadError> {
        validate_path_id(&request.cycle_id)?;
        validate_page_request(
            request.page_size,
            request.page_token.as_deref(),
            request.lang.as_deref(),
        )?;
        let raw = self
            .http_client
            .send_json(build_list_cycle_objectives_request(&self.config, request))
            .map_err(FeishuOkrReadError::from)?;
        map_status_or_parse_cycle_objectives_list(raw.status, &raw.body)
    }

    pub fn list_objective_key_results(
        &mut self,
        request: FeishuOkrObjectiveKeyResultsListRequest,
    ) -> Result<FeishuOkrObjectiveKeyResultsListResponse, FeishuOkrReadError> {
        validate_path_id(&request.objective_id)?;
        validate_page_request(
            request.page_size,
            request.page_token.as_deref(),
            request.lang.as_deref(),
        )?;
        let raw = self
            .http_client
            .send_json(build_list_objective_key_results_request(
                &self.config,
                request,
            ))
            .map_err(FeishuOkrReadError::from)?;
        map_status_or_parse_objective_key_results_list(raw.status, &raw.body)
    }

    pub fn list_progress(
        &mut self,
        request: FeishuOkrProgressListRequest,
    ) -> Result<FeishuOkrProgressListResponse, FeishuOkrReadError> {
        validate_progress_list_request(&request)?;
        let raw = self
            .http_client
            .send_json(build_progress_list_request(&self.config, request))
            .map_err(FeishuOkrReadError::from)?;
        map_status_or_parse_progress_list(raw.status, &raw.body)
    }
}

#[async_trait]
pub trait AsyncFeishuOkrRead {
    async fn batch_get_okrs(
        &mut self,
        request: FeishuOkrBatchGetRequest,
    ) -> Result<FeishuOkrBatchGetResponse, FeishuOkrReadError>;

    async fn list_cycles(
        &mut self,
        request: FeishuOkrCycleListRequest,
    ) -> Result<FeishuOkrCycleListResponse, FeishuOkrReadError>;

    async fn list_cycle_objectives(
        &mut self,
        request: FeishuOkrCycleObjectivesListRequest,
    ) -> Result<FeishuOkrCycleObjectivesListResponse, FeishuOkrReadError>;

    async fn list_objective_key_results(
        &mut self,
        request: FeishuOkrObjectiveKeyResultsListRequest,
    ) -> Result<FeishuOkrObjectiveKeyResultsListResponse, FeishuOkrReadError>;

    async fn list_progress(
        &mut self,
        request: FeishuOkrProgressListRequest,
    ) -> Result<FeishuOkrProgressListResponse, FeishuOkrReadError>;
}

#[async_trait]
impl<H> AsyncFeishuOkrRead for FeishuOkrReadClient<H>
where
    H: AsyncHttpClient + Send,
{
    async fn batch_get_okrs(
        &mut self,
        request: FeishuOkrBatchGetRequest,
    ) -> Result<FeishuOkrBatchGetResponse, FeishuOkrReadError> {
        if request.okr_ids.is_empty() || request.okr_ids.len() > 10 {
            return Err(FeishuOkrReadError::InvalidRequest);
        }
        let raw = self
            .http_client
            .send_json(build_batch_get_okr_request(&self.config, request))
            .await
            .map_err(FeishuOkrReadError::from)?;
        map_status_or_parse_batch_get(raw.status, &raw.body)
    }

    async fn list_cycles(
        &mut self,
        request: FeishuOkrCycleListRequest,
    ) -> Result<FeishuOkrCycleListResponse, FeishuOkrReadError> {
        validate_path_id(&request.user_id)?;
        validate_page_request(
            request.page_size,
            request.page_token.as_deref(),
            request.lang.as_deref(),
        )?;
        let raw = self
            .http_client
            .send_json(build_list_cycles_request(&self.config, request))
            .await
            .map_err(FeishuOkrReadError::from)?;
        map_status_or_parse_cycle_list(raw.status, &raw.body)
    }

    async fn list_cycle_objectives(
        &mut self,
        request: FeishuOkrCycleObjectivesListRequest,
    ) -> Result<FeishuOkrCycleObjectivesListResponse, FeishuOkrReadError> {
        validate_path_id(&request.cycle_id)?;
        validate_page_request(
            request.page_size,
            request.page_token.as_deref(),
            request.lang.as_deref(),
        )?;
        let raw = self
            .http_client
            .send_json(build_list_cycle_objectives_request(&self.config, request))
            .await
            .map_err(FeishuOkrReadError::from)?;
        map_status_or_parse_cycle_objectives_list(raw.status, &raw.body)
    }

    async fn list_objective_key_results(
        &mut self,
        request: FeishuOkrObjectiveKeyResultsListRequest,
    ) -> Result<FeishuOkrObjectiveKeyResultsListResponse, FeishuOkrReadError> {
        validate_path_id(&request.objective_id)?;
        validate_page_request(
            request.page_size,
            request.page_token.as_deref(),
            request.lang.as_deref(),
        )?;
        let raw = self
            .http_client
            .send_json(build_list_objective_key_results_request(
                &self.config,
                request,
            ))
            .await
            .map_err(FeishuOkrReadError::from)?;
        map_status_or_parse_objective_key_results_list(raw.status, &raw.body)
    }

    async fn list_progress(
        &mut self,
        request: FeishuOkrProgressListRequest,
    ) -> Result<FeishuOkrProgressListResponse, FeishuOkrReadError> {
        validate_progress_list_request(&request)?;
        let raw = self
            .http_client
            .send_json(build_progress_list_request(&self.config, request))
            .await
            .map_err(FeishuOkrReadError::from)?;
        map_status_or_parse_progress_list(raw.status, &raw.body)
    }
}

pub fn build_batch_get_okr_request(
    config: &FeishuOpenApiConfig,
    request: FeishuOkrBatchGetRequest,
) -> HttpRequest {
    let mut query = vec![("user_id_type", request.user_id_type.as_str().to_string())];
    for okr_id in request.okr_ids {
        query.push(("okr_ids", okr_id));
    }
    if let Some(lang) = request.lang {
        query.push(("lang", lang));
    }
    let query_string = encode_query(&query);
    let url = format!(
        "{}{}?{}",
        config.base_url.trim_end_matches('/'),
        OKR_BATCH_GET_PATH,
        query_string
    );

    HttpRequest {
        method: "GET".to_string(),
        url,
        headers: vec![
            (
                "Authorization".to_string(),
                format!("Bearer {}", request.user_access_token.expose_secret()),
            ),
            ("Accept".to_string(), "application/json".to_string()),
            ("User-Agent".to_string(), OAR_USER_AGENT.to_string()),
        ],
        body: Value::Object(serde_json::Map::new()),
        max_response_bytes: config.max_response_bytes,
    }
}

pub fn build_list_cycles_request(
    config: &FeishuOpenApiConfig,
    request: FeishuOkrCycleListRequest,
) -> HttpRequest {
    let mut query = vec![
        ("user_id_type", request.user_id_type.as_str().to_string()),
        ("user_id", request.user_id),
    ];
    query.extend(build_page_query(
        request.page_size,
        request.page_token,
        request.lang,
    ));
    build_get_request(
        config,
        OKR_CYCLES_PATH.to_string(),
        query,
        request.user_access_token,
    )
}

pub fn build_list_cycle_objectives_request(
    config: &FeishuOpenApiConfig,
    request: FeishuOkrCycleObjectivesListRequest,
) -> HttpRequest {
    let path = format!(
        "{}/{}/objectives",
        OKR_CYCLES_PATH,
        percent_encode(&request.cycle_id)
    );
    let mut query = vec![("user_id_type", request.user_id_type.as_str().to_string())];
    query.extend(build_page_query(
        request.page_size,
        request.page_token,
        request.lang,
    ));
    build_get_request(config, path, query, request.user_access_token)
}

pub fn build_list_objective_key_results_request(
    config: &FeishuOpenApiConfig,
    request: FeishuOkrObjectiveKeyResultsListRequest,
) -> HttpRequest {
    let path = format!(
        "{}/{}/key_results",
        OKR_OBJECTIVES_PATH,
        percent_encode(&request.objective_id)
    );
    let mut query = vec![("user_id_type", request.user_id_type.as_str().to_string())];
    query.extend(build_page_query(
        request.page_size,
        request.page_token,
        request.lang,
    ));
    build_get_request(config, path, query, request.user_access_token)
}

pub fn build_progress_list_request(
    config: &FeishuOpenApiConfig,
    request: FeishuOkrProgressListRequest,
) -> HttpRequest {
    let path = match &request.target {
        FeishuOkrProgressListTarget::Objective(objective_id) => format!(
            "{}/{}/progresses",
            OKR_OBJECTIVES_PATH,
            percent_encode(objective_id)
        ),
        FeishuOkrProgressListTarget::KeyResult(key_result_id) => format!(
            "{}/{}/progresses",
            OKR_KEY_RESULTS_PATH,
            percent_encode(key_result_id)
        ),
    };
    let mut query = vec![
        ("user_id_type", request.user_id_type.as_str().to_string()),
        (
            "department_id_type",
            request.department_id_type.as_str().to_string(),
        ),
        (
            "page_size",
            request.page_size.unwrap_or(DEFAULT_PAGE_SIZE).to_string(),
        ),
    ];
    if let Some(page_token) = request.page_token {
        query.push(("page_token", page_token));
    }
    build_get_request(config, path, query, request.user_access_token)
}

fn build_get_request(
    config: &FeishuOpenApiConfig,
    path: String,
    query: Vec<(&str, String)>,
    user_access_token: crate::redaction::SecretString,
) -> HttpRequest {
    let query_string = encode_query(&query);
    let url = if query_string.is_empty() {
        format!("{}{}", config.base_url.trim_end_matches('/'), path)
    } else {
        format!(
            "{}{}?{}",
            config.base_url.trim_end_matches('/'),
            path,
            query_string
        )
    };

    HttpRequest {
        method: "GET".to_string(),
        url,
        headers: vec![
            (
                "Authorization".to_string(),
                format!("Bearer {}", user_access_token.expose_secret()),
            ),
            ("Accept".to_string(), "application/json".to_string()),
            ("User-Agent".to_string(), OAR_USER_AGENT.to_string()),
        ],
        body: Value::Object(serde_json::Map::new()),
        max_response_bytes: config.max_response_bytes,
    }
}

fn build_page_query(
    page_size: Option<u32>,
    page_token: Option<String>,
    lang: Option<String>,
) -> Vec<(&'static str, String)> {
    let mut query = Vec::new();
    if let Some(page_size) = page_size {
        query.push(("page_size", page_size.to_string()));
    }
    if let Some(page_token) = page_token {
        query.push(("page_token", page_token));
    }
    if let Some(lang) = lang {
        query.push(("lang", lang));
    }
    query
}

fn map_status_or_parse_batch_get(
    status: u16,
    body: &str,
) -> Result<FeishuOkrBatchGetResponse, FeishuOkrReadError> {
    map_status_or_parse_okr_response(status, body)
}

fn map_status_or_parse_cycle_list(
    status: u16,
    body: &str,
) -> Result<FeishuOkrCycleListResponse, FeishuOkrReadError> {
    map_status_or_parse_okr_response(status, body)
}

fn map_status_or_parse_cycle_objectives_list(
    status: u16,
    body: &str,
) -> Result<FeishuOkrCycleObjectivesListResponse, FeishuOkrReadError> {
    map_status_or_parse_okr_response(status, body)
}

fn map_status_or_parse_objective_key_results_list(
    status: u16,
    body: &str,
) -> Result<FeishuOkrObjectiveKeyResultsListResponse, FeishuOkrReadError> {
    map_status_or_parse_okr_response(status, body)
}

fn map_status_or_parse_progress_list(
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

fn validate_page_request(
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

fn validate_progress_list_request(
    request: &FeishuOkrProgressListRequest,
) -> Result<(), FeishuOkrReadError> {
    validate_path_id(request.target.id())?;
    let page_size = request.page_size.unwrap_or(DEFAULT_PAGE_SIZE);
    if page_size == 0 || page_size > MAX_PAGE_SIZE {
        return Err(FeishuOkrReadError::InvalidRequest);
    }
    validate_non_empty_optional_len(request.page_token.as_deref(), MAX_PAGE_TOKEN_BYTES)?;
    Ok(())
}

fn validate_path_id(value: &str) -> Result<(), FeishuOkrReadError> {
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

fn encode_query(parts: &[(&str, String)]) -> String {
    parts
        .iter()
        .map(|(k, v)| format!("{}={}", percent_encode(k), percent_encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.as_bytes() {
        if byte.is_ascii_alphanumeric() || [b'-', b'_', b'.', b'~'].contains(byte) {
            out.push(*byte as char);
        } else {
            out.push('%');
            out.push_str(&format!("{:02X}", byte));
        }
    }
    out
}

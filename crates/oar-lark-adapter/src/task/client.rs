use async_trait::async_trait;
use serde_json::Value;

use crate::config::FeishuOpenApiConfig;
use crate::oauth::{AsyncHttpClient, HttpClient, HttpRequest};

use super::error::FeishuTaskReadError;
use super::types::{FeishuTaskGetRequest, FeishuTaskGetResponse, TaskReadSummary, TaskSourceRef};

const TASK_GET_PATH_PREFIX: &str = "/open-apis/task/v2/tasks";
const OAR_USER_AGENT: &str = concat!("oar-lark-adapter/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Clone)]
pub struct FeishuTaskReadClient<H> {
    config: FeishuOpenApiConfig,
    http_client: H,
}

impl<H> FeishuTaskReadClient<H> {
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

impl<H> FeishuTaskReadClient<H>
where
    H: HttpClient,
{
    pub fn get_task_summary(
        &mut self,
        request: FeishuTaskGetRequest,
    ) -> Result<TaskReadSummary, FeishuTaskReadError> {
        let source_ref = parse_task_source_ref(&request.source_ref)?;
        let raw = self
            .http_client
            .send_json(build_get_task_request(&self.config, request, &source_ref))
            .map_err(FeishuTaskReadError::from)?;
        map_status_or_parse_task(raw.status, &raw.body, &source_ref)
    }
}

#[async_trait]
pub trait AsyncFeishuTaskRead {
    async fn get_task_summary(
        &mut self,
        request: FeishuTaskGetRequest,
    ) -> Result<TaskReadSummary, FeishuTaskReadError>;
}

#[async_trait]
impl<H> AsyncFeishuTaskRead for FeishuTaskReadClient<H>
where
    H: AsyncHttpClient + Send,
{
    async fn get_task_summary(
        &mut self,
        request: FeishuTaskGetRequest,
    ) -> Result<TaskReadSummary, FeishuTaskReadError> {
        let source_ref = parse_task_source_ref(&request.source_ref)?;
        let raw = self
            .http_client
            .send_json(build_get_task_request(&self.config, request, &source_ref))
            .await
            .map_err(FeishuTaskReadError::from)?;
        map_status_or_parse_task(raw.status, &raw.body, &source_ref)
    }
}

pub fn parse_task_source_ref(source_ref: &str) -> Result<TaskSourceRef, FeishuTaskReadError> {
    let trimmed = source_ref.trim();
    let task_id = if let Some(task_id) = trimmed.strip_prefix("task://") {
        task_id
    } else if let Some(task_id) = trimmed.strip_prefix("feishu://task/") {
        task_id
    } else {
        return Err(FeishuTaskReadError::InvalidSourceRef);
    };
    if task_id.is_empty()
        || task_id.len() > 100
        || task_id.contains('/')
        || task_id.contains('?')
        || task_id.contains('#')
    {
        return Err(FeishuTaskReadError::InvalidSourceRef);
    }
    Ok(TaskSourceRef {
        task_id: task_id.to_string(),
    })
}

pub fn build_get_task_request(
    config: &FeishuOpenApiConfig,
    request: FeishuTaskGetRequest,
    source_ref: &TaskSourceRef,
) -> HttpRequest {
    let query_string = encode_query(&[("user_id_type", request.user_id_type.as_str().to_string())]);
    // Feishu Task v2 task detail endpoint; read access is gated by task:task:read.
    let url = format!(
        "{}/{}/{}?{}",
        config.base_url.trim_end_matches('/'),
        TASK_GET_PATH_PREFIX.trim_start_matches('/'),
        percent_encode(&source_ref.task_id),
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

fn map_status_or_parse_task(
    status: u16,
    body: &str,
    source_ref: &TaskSourceRef,
) -> Result<TaskReadSummary, FeishuTaskReadError> {
    match status {
        200..=299 => {
            let parsed: FeishuTaskGetResponse =
                serde_json::from_str(body).map_err(|_| FeishuTaskReadError::InvalidJson)?;
            if parsed.code != 0 {
                return Err(map_api_code(parsed.code));
            }
            let task = parsed
                .data
                .and_then(|data| data.task)
                .ok_or(FeishuTaskReadError::InvalidJson)?;
            Ok(TaskReadSummary::from_feishu_task(source_ref, task))
        }
        401 => Err(FeishuTaskReadError::Unauthorized),
        403 => Err(FeishuTaskReadError::Forbidden),
        404 => Err(FeishuTaskReadError::NotFound),
        400..=499 => Err(FeishuTaskReadError::UpstreamClient),
        _ => Err(FeishuTaskReadError::UpstreamTransient),
    }
}

fn map_api_code(code: i64) -> FeishuTaskReadError {
    match code {
        401 | 99991663 | 99991664 => FeishuTaskReadError::Unauthorized,
        403 | 1470403 => FeishuTaskReadError::Forbidden,
        404 | 1470404 => FeishuTaskReadError::NotFound,
        400..=499 => FeishuTaskReadError::UpstreamClient,
        _ => FeishuTaskReadError::ApiFailure,
    }
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

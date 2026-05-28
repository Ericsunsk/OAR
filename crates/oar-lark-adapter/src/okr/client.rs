use async_trait::async_trait;
use serde_json::Value;

use crate::config::FeishuOpenApiConfig;
use crate::oauth::{AsyncHttpClient, HttpClient, HttpRequest};

use super::error::FeishuOkrReadError;
use super::types::{FeishuOkrBatchGetRequest, FeishuOkrBatchGetResponse};

const OKR_BATCH_GET_PATH: &str = "/open-apis/okr/v1/okrs/batch_get";
const OKR_PROGRESS_RECORDS_PATH: &str = "/open-apis/okr/v1/progress_records";
const OAR_USER_AGENT: &str = concat!("oar-lark-adapter/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Clone)]
pub struct OkrProgressListRequest {
    pub user_id_type: super::types::OkrUserIdType,
    pub user_id: String,
    pub page_size: Option<u32>,
    pub page_token: Option<String>,
    pub lang: Option<String>,
}

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
}

#[async_trait]
pub trait AsyncFeishuOkrRead {
    async fn batch_get_okrs(
        &mut self,
        request: FeishuOkrBatchGetRequest,
    ) -> Result<FeishuOkrBatchGetResponse, FeishuOkrReadError>;
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

pub fn build_progress_list_request(
    config: &FeishuOpenApiConfig,
    user_access_token: crate::redaction::SecretString,
    request: OkrProgressListRequest,
) -> HttpRequest {
    let mut query = vec![
        ("user_id_type", request.user_id_type.as_str().to_string()),
        ("user_id", request.user_id),
    ];
    if let Some(page_size) = request.page_size {
        query.push(("page_size", page_size.to_string()));
    }
    if let Some(page_token) = request.page_token {
        query.push(("page_token", page_token));
    }
    if let Some(lang) = request.lang {
        query.push(("lang", lang));
    }

    // TODO: Confirm final query/body schema in official API Explorer before enabling this in
    // production read path; the transport boundary and parser are prepared for staged rollout.
    let url = format!(
        "{}{}?{}",
        config.base_url.trim_end_matches('/'),
        OKR_PROGRESS_RECORDS_PATH,
        encode_query(&query)
    );

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

fn map_status_or_parse_batch_get(
    status: u16,
    body: &str,
) -> Result<FeishuOkrBatchGetResponse, FeishuOkrReadError> {
    match status {
        200..=299 => {
            let parsed: FeishuOkrBatchGetResponse =
                serde_json::from_str(body).map_err(|_| FeishuOkrReadError::InvalidJson)?;
            if parsed.code != 0 {
                return Err(FeishuOkrReadError::ApiFailure);
            }
            Ok(parsed)
        }
        401 => Err(FeishuOkrReadError::Unauthorized),
        403 => Err(FeishuOkrReadError::Forbidden),
        400..=499 => Err(FeishuOkrReadError::UpstreamClient),
        _ => Err(FeishuOkrReadError::UpstreamTransient),
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

use std::fmt;
use std::io::Read;
use std::time::Duration;

use async_trait::async_trait;

use crate::config::FeishuOpenApiConfig;

pub trait HttpClient {
    fn post_json(&mut self, request: HttpRequest) -> Result<HttpResponse, HttpClientFailure>;
}

#[async_trait(?Send)]
pub trait AsyncHttpClient {
    async fn post_json(&mut self, request: HttpRequest) -> Result<HttpResponse, HttpClientFailure>;
}

#[derive(Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: serde_json::Value,
    pub max_response_bytes: usize,
}

impl fmt::Debug for HttpRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpRequest")
            .field("method", &self.method)
            .field("url", &self.url)
            .field("headers", &self.headers)
            .field("body", &"[REDACTED]")
            .field("max_response_bytes", &self.max_response_bytes)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

impl HttpResponse {
    pub fn new(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            body: body.into(),
        }
    }
}

impl fmt::Debug for HttpResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpResponse")
            .field("status", &self.status)
            .field("body", &"[REDACTED]")
            .field("body_len", &self.body.len())
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum HttpClientFailure {
    Transport,
    OversizedResponse { max_response_bytes: usize },
}

impl fmt::Debug for HttpClientFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HttpClientFailure::Transport => write!(f, "HttpClientFailure(transport)"),
            HttpClientFailure::OversizedResponse { max_response_bytes } => write!(
                f,
                "HttpClientFailure(oversized_response max={}B)",
                max_response_bytes
            ),
        }
    }
}

impl fmt::Display for HttpClientFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HttpClientFailure::Transport => write!(f, "feishu http transport failed"),
            HttpClientFailure::OversizedResponse { max_response_bytes } => write!(
                f,
                "feishu http response exceeded {} bytes",
                max_response_bytes
            ),
        }
    }
}

impl std::error::Error for HttpClientFailure {}

macro_rules! apply_headers {
    ($builder:expr, $headers:expr) => {{
        let mut builder = $builder;
        for (name, value) in $headers {
            builder = builder.header(name.as_str(), value.as_str());
        }
        builder
    }};
}

#[derive(Debug, Clone, Default)]
pub struct ReqwestBlockingHttpClient {
    client: reqwest::blocking::Client,
}

impl ReqwestBlockingHttpClient {
    pub fn new() -> Self {
        Self::with_config(&FeishuOpenApiConfig::default())
            .expect("default Feishu reqwest client config should be valid")
    }

    pub fn with_config(config: &FeishuOpenApiConfig) -> Result<Self, HttpClientFailure> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(config.request_timeout_ms))
            .connect_timeout(Duration::from_millis(config.connect_timeout_ms))
            .build()
            .map_err(|_| HttpClientFailure::Transport)?;
        Ok(Self { client })
    }
}

impl HttpClient for ReqwestBlockingHttpClient {
    fn post_json(&mut self, request: HttpRequest) -> Result<HttpResponse, HttpClientFailure> {
        let builder = apply_headers!(self.client.post(&request.url), &request.headers);
        let response = builder
            .json(&request.body)
            .send()
            .map_err(|_| HttpClientFailure::Transport)?;
        let status = response.status().as_u16();
        let mut body = String::new();
        let max_read = request.max_response_bytes.saturating_add(1) as u64;
        response
            .take(max_read)
            .read_to_string(&mut body)
            .map_err(|_| HttpClientFailure::Transport)?;
        if body.len() > request.max_response_bytes {
            return Err(HttpClientFailure::OversizedResponse {
                max_response_bytes: request.max_response_bytes,
            });
        }
        Ok(HttpResponse { status, body })
    }
}

#[derive(Debug, Clone, Default)]
pub struct ReqwestAsyncHttpClient {
    client: reqwest::Client,
}

impl ReqwestAsyncHttpClient {
    pub fn new() -> Self {
        Self::with_config(&FeishuOpenApiConfig::default())
            .expect("default Feishu reqwest client config should be valid")
    }

    pub fn with_config(config: &FeishuOpenApiConfig) -> Result<Self, HttpClientFailure> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(config.request_timeout_ms))
            .connect_timeout(Duration::from_millis(config.connect_timeout_ms))
            .build()
            .map_err(|_| HttpClientFailure::Transport)?;
        Ok(Self { client })
    }
}

#[async_trait(?Send)]
impl AsyncHttpClient for ReqwestAsyncHttpClient {
    async fn post_json(&mut self, request: HttpRequest) -> Result<HttpResponse, HttpClientFailure> {
        let builder = apply_headers!(self.client.post(&request.url), &request.headers);
        let response = builder
            .json(&request.body)
            .send()
            .await
            .map_err(|_| HttpClientFailure::Transport)?;
        let status = response.status().as_u16();
        let bytes = response
            .bytes()
            .await
            .map_err(|_| HttpClientFailure::Transport)?;
        if bytes.len() > request.max_response_bytes {
            return Err(HttpClientFailure::OversizedResponse {
                max_response_bytes: request.max_response_bytes,
            });
        }
        let body = String::from_utf8(bytes.to_vec()).map_err(|_| HttpClientFailure::Transport)?;
        Ok(HttpResponse { status, body })
    }
}

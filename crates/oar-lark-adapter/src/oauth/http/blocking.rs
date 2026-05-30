use std::io::Read;
use std::time::Duration;

use crate::config::FeishuOpenApiConfig;

use super::types::{HttpClient, HttpClientFailure, HttpRequest, HttpResponse};

#[derive(Debug, Clone, Default)]
pub struct ReqwestBlockingHttpClient {
    client: reqwest::blocking::Client,
}

impl ReqwestBlockingHttpClient {
    pub fn new() -> Self {
        match Self::with_config(&FeishuOpenApiConfig::default()) {
            Ok(client) => client,
            Err(error) => {
                tracing::warn!(?error, "failed to build configured blocking http client");
                Self {
                    client: reqwest::blocking::Client::new(),
                }
            }
        }
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
    fn post_json(&mut self, mut request: HttpRequest) -> Result<HttpResponse, HttpClientFailure> {
        request.method = "POST".to_string();
        self.send_json(request)
    }

    fn send_json(&mut self, request: HttpRequest) -> Result<HttpResponse, HttpClientFailure> {
        let method = request.method.to_ascii_uppercase();
        let builder = match method.as_str() {
            "GET" => apply_headers!(self.client.get(&request.url), &request.headers),
            "POST" => {
                apply_headers!(self.client.post(&request.url), &request.headers).json(&request.body)
            }
            _ => return Err(HttpClientFailure::Transport),
        };
        let response = builder.send().map_err(|_| HttpClientFailure::Transport)?;
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

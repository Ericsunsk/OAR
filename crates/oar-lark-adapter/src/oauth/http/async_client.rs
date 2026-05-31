use std::time::Duration;

use async_trait::async_trait;

use crate::config::FeishuOpenApiConfig;

use super::types::{AsyncHttpClient, HttpClientFailure, HttpRequest, HttpResponse};

#[derive(Debug, Clone, Default)]
pub struct ReqwestAsyncHttpClient {
    client: reqwest::Client,
}

impl ReqwestAsyncHttpClient {
    pub fn new() -> Self {
        match Self::with_config(&FeishuOpenApiConfig::default()) {
            Ok(client) => client,
            Err(error) => {
                tracing::warn!(?error, "failed to build configured async http client");
                Self {
                    client: reqwest::Client::new(),
                }
            }
        }
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

#[async_trait]
impl AsyncHttpClient for ReqwestAsyncHttpClient {
    async fn post_json(
        &mut self,
        mut request: HttpRequest,
    ) -> Result<HttpResponse, HttpClientFailure> {
        request.method = "POST".to_string();
        self.send_json(request).await
    }

    async fn send_json(&mut self, request: HttpRequest) -> Result<HttpResponse, HttpClientFailure> {
        let method = request.method.to_ascii_uppercase();
        let builder = match method.as_str() {
            "GET" => apply_headers!(self.client.get(&request.url), &request.headers),
            "POST" => {
                apply_headers!(self.client.post(&request.url), &request.headers).json(&request.body)
            }
            _ => return Err(HttpClientFailure::Transport),
        };
        let mut response = builder
            .send()
            .await
            .map_err(|_| HttpClientFailure::Transport)?;
        let status = response.status().as_u16();
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| HttpClientFailure::Transport)?
        {
            if body.len().saturating_add(chunk.len()) > request.max_response_bytes {
                return Err(HttpClientFailure::OversizedResponse {
                    max_response_bytes: request.max_response_bytes,
                });
            }
            body.extend_from_slice(&chunk);
        }
        let body = String::from_utf8(body).map_err(|_| HttpClientFailure::Transport)?;
        Ok(HttpResponse { status, body })
    }
}

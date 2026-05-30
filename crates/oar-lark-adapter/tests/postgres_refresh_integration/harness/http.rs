use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use oar_lark_adapter::oauth::{HttpClientFailure, HttpRequest};
use oar_lark_adapter::{AsyncHttpClient, HttpResponse};

use super::constants::{NEW_ACCESS_TOKEN, NEW_REFRESH_TOKEN};

#[derive(Clone)]
pub(crate) struct RecordingAsyncHttpClient {
    result: Result<HttpResponse, HttpClientFailure>,
    requests: Arc<Mutex<Vec<HttpRequest>>>,
}

impl RecordingAsyncHttpClient {
    pub(crate) fn from_response(response: HttpResponse) -> Self {
        Self::from_result(Ok(response))
    }

    pub(crate) fn from_result(result: Result<HttpResponse, HttpClientFailure>) -> Self {
        Self {
            result,
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn requests(&self) -> Vec<HttpRequest> {
        self.requests
            .lock()
            .expect("fake http request mutex")
            .clone()
    }
}

#[async_trait]
impl AsyncHttpClient for RecordingAsyncHttpClient {
    async fn post_json(&mut self, request: HttpRequest) -> Result<HttpResponse, HttpClientFailure> {
        self.requests
            .lock()
            .expect("fake http request mutex")
            .push(request);
        self.result.clone()
    }
}

pub(crate) fn success_body() -> String {
    serde_json::json!({
        "code": 0,
        "access_token": NEW_ACCESS_TOKEN,
        "expires_in": 7200,
        "refresh_token": NEW_REFRESH_TOKEN,
        "refresh_token_expires_in": 604800,
        "scope": "offline_access auth:user.id:read okr.progress.write",
        "token_type": "Bearer"
    })
    .to_string()
}

pub(crate) fn failure_body(code: i64) -> String {
    serde_json::json!({
        "code": code,
        "error": "server_error",
        "error_description": "redacted"
    })
    .to_string()
}

pub(crate) fn assert_feishu_refresh_headers(headers: &[(String, String)]) {
    assert_eq!(
        headers,
        &[
            (
                "Content-Type".to_string(),
                "application/json; charset=utf-8".to_string()
            ),
            ("Accept".to_string(), "application/json".to_string()),
            (
                "User-Agent".to_string(),
                format!("oar-lark-adapter/{}", env!("CARGO_PKG_VERSION"))
            )
        ]
    );
}

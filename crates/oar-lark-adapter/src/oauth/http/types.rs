use std::fmt;

use async_trait::async_trait;

pub trait HttpClient {
    fn post_json(&mut self, request: HttpRequest) -> Result<HttpResponse, HttpClientFailure>;

    fn send_json(&mut self, request: HttpRequest) -> Result<HttpResponse, HttpClientFailure> {
        self.post_json(request)
    }
}

#[async_trait]
pub trait AsyncHttpClient {
    async fn post_json(&mut self, request: HttpRequest) -> Result<HttpResponse, HttpClientFailure>;

    async fn send_json(&mut self, request: HttpRequest) -> Result<HttpResponse, HttpClientFailure> {
        self.post_json(request).await
    }
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
        let headers = self
            .headers
            .iter()
            .map(|(name, value)| {
                if is_sensitive_header(name) {
                    (name.clone(), "[REDACTED]".to_string())
                } else {
                    (name.clone(), value.clone())
                }
            })
            .collect::<Vec<_>>();
        f.debug_struct("HttpRequest")
            .field("method", &self.method)
            .field("url", &redacted_url(&self.url))
            .field("headers", &headers)
            .field("body", &"[REDACTED]")
            .field("max_response_bytes", &self.max_response_bytes)
            .finish()
    }
}

fn is_sensitive_header(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower == "authorization"
        || lower == "cookie"
        || lower == "set-cookie"
        || lower.starts_with("x-lark-")
}

fn redacted_url(value: &str) -> String {
    let Ok(mut url) = reqwest::Url::parse(value) else {
        return "[REDACTED_URL]".to_string();
    };
    url.set_query(None);
    url.set_fragment(None);
    let Some(host) = url.host_str() else {
        return "[REDACTED_URL]".to_string();
    };
    let port = url
        .port()
        .map(|port| format!(":{port}"))
        .unwrap_or_default();
    format!("{}://{}{}/[REDACTED_PATH]", url.scheme(), host, port)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_debug_redacts_path_query_fragment_and_body() {
        let request = HttpRequest {
            method: "GET".to_string(),
            url: "https://open.feishu.cn/open-apis/calendar/v4/calendars/cal_secret/events/evt_secret?token=secret#frag"
                .to_string(),
            headers: vec![(
                "Authorization".to_string(),
                "Bearer u-very-secret-token".to_string(),
            )],
            body: serde_json::json!({"secret": "body-secret"}),
            max_response_bytes: 1024,
        };

        let debug = format!("{request:?}");

        assert!(debug.contains("https://open.feishu.cn/[REDACTED_PATH]"));
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("cal_secret"));
        assert!(!debug.contains("evt_secret"));
        assert!(!debug.contains("token=secret"));
        assert!(!debug.contains("body-secret"));
        assert!(!debug.contains("u-very-secret-token"));
    }
}

impl std::error::Error for HttpClientFailure {}

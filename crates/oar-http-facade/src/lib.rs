#![forbid(unsafe_code)]

use std::convert::Infallible;
use std::error::Error;
use std::fmt;
use std::net::SocketAddr;

use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::header::{ACCEPT, AUTHORIZATION, CACHE_CONTROL, CONTENT_TYPE};
use hyper::http::{HeaderValue, Method, StatusCode};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tracing::{error, info};

type ResponseBody = Full<Bytes>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OarHttpFacadeConfig {
    pub bind_addr: SocketAddr,
}

impl Default for OarHttpFacadeConfig {
    fn default() -> Self {
        Self {
            bind_addr: SocketAddr::from(([127, 0, 0, 1], 8080)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OarHttpFacadeConfigError {
    InvalidBindAddr,
}

impl fmt::Display for OarHttpFacadeConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBindAddr => {
                write!(f, "oar_http_facade_config_invalid: invalid_bind_addr")
            }
        }
    }
}

impl Error for OarHttpFacadeConfigError {}

impl OarHttpFacadeConfig {
    pub fn from_env_map(
        env: &impl Fn(&str) -> Option<String>,
    ) -> Result<Self, OarHttpFacadeConfigError> {
        let Some(raw_bind_addr) = env("OAR_HTTP_BIND_ADDR") else {
            return Ok(Self::default());
        };
        let bind_addr = raw_bind_addr
            .parse::<SocketAddr>()
            .map_err(|_| OarHttpFacadeConfigError::InvalidBindAddr)?;
        Ok(Self { bind_addr })
    }
}

#[derive(Debug)]
pub enum OarHttpFacadeError {
    Bind(std::io::Error),
    Accept(std::io::Error),
}

impl fmt::Display for OarHttpFacadeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bind(_) => write!(f, "oar_http_facade_bind_failed"),
            Self::Accept(_) => write!(f, "oar_http_facade_accept_failed"),
        }
    }
}

impl Error for OarHttpFacadeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Bind(error) | Self::Accept(error) => Some(error),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FacadeResponse {
    pub status: StatusCode,
    pub content_type: &'static str,
    pub body: String,
}

pub async fn run(config: OarHttpFacadeConfig) -> Result<(), OarHttpFacadeError> {
    let listener = TcpListener::bind(config.bind_addr)
        .await
        .map_err(OarHttpFacadeError::Bind)?;
    info!(bind_addr = %config.bind_addr, "oar http facade listening");

    loop {
        let (stream, remote_addr) = listener
            .accept()
            .await
            .map_err(OarHttpFacadeError::Accept)?;
        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            if let Err(error) = http1::Builder::new()
                .serve_connection(io, service_fn(handle_hyper_request))
                .await
            {
                error!(?error, %remote_addr, "oar http facade connection failed");
            }
        });
    }
}

pub async fn handle_hyper_request(
    request: Request<Incoming>,
) -> Result<Response<ResponseBody>, Infallible> {
    let authorization = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok());
    let accept = request
        .headers()
        .get(ACCEPT)
        .and_then(|value| value.to_str().ok());
    let facade_response = dispatch_request(
        request.method(),
        request.uri().path(),
        authorization,
        accept,
    );
    Ok(facade_response.into_hyper_response())
}

pub fn dispatch_request(
    method: &Method,
    path: &str,
    authorization: Option<&str>,
    accept: Option<&str>,
) -> FacadeResponse {
    match (method, path) {
        (&Method::GET, "/healthz") => json_facade_response(
            StatusCode::OK,
            json!({
                "status": "ok",
                "service": "oar-http-facade"
            }),
        ),
        (&Method::POST, "/auth/feishu/qr-sessions") => service_unavailable(
            "feishu_auth_not_configured",
            "Feishu QR login is not configured in this backend facade.",
        ),
        (&Method::POST, "/auth/logout") => not_implemented(
            "auth_logout_not_wired",
            "Logout is not wired until real session storage is connected.",
        ),
        (&Method::GET, "/review-inbox/snapshot") => {
            if !has_oar_session(authorization) {
                return unauthorized();
            }
            json_facade_response(StatusCode::OK, empty_review_inbox_snapshot())
        }
        (&Method::POST, "/review-inbox/decisions") => {
            if !has_oar_session(authorization) {
                return unauthorized();
            }
            json_facade_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                json!({
                    "error": "review_decision_not_wired",
                    "safe_message": "Review decisions are disabled until the ConfirmedAction ledger path is connected."
                }),
            )
        }
        _ if is_auth_session_status_route(method, path) => service_unavailable(
            "feishu_auth_not_configured",
            "Feishu QR login is not configured in this backend facade.",
        ),
        _ if is_auth_session_events_route(method, path) => {
            if accept != Some("text/event-stream") {
                return json_facade_response(
                    StatusCode::NOT_ACCEPTABLE,
                    json!({
                        "error": "event_stream_required",
                        "safe_message": "Auth session events require Accept: text/event-stream."
                    }),
                );
            }
            service_unavailable(
                "feishu_auth_not_configured",
                "Feishu QR login events are not configured in this backend facade.",
            )
        }
        _ => json_facade_response(
            StatusCode::NOT_FOUND,
            json!({
                "error": "not_found",
                "safe_message": "No OAR backend route matched this request."
            }),
        ),
    }
}

impl FacadeResponse {
    fn into_hyper_response(self) -> Response<ResponseBody> {
        let mut response = Response::new(Full::new(Bytes::from(self.body)));
        *response.status_mut() = self.status;
        response
            .headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static(self.content_type));
        response
            .headers_mut()
            .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
        response
    }
}

fn empty_review_inbox_snapshot() -> Value {
    json!({
        "contract_version": 1,
        "generated_at": "1970-01-01T00:00:00Z",
        "items": [],
        "proposed_actions": [],
        "evidence": [],
        "ledger_events": []
    })
}

fn json_facade_response(status: StatusCode, body: Value) -> FacadeResponse {
    FacadeResponse {
        status,
        content_type: "application/json",
        body: body.to_string(),
    }
}

fn service_unavailable(error: &'static str, safe_message: &'static str) -> FacadeResponse {
    json_facade_response(
        StatusCode::SERVICE_UNAVAILABLE,
        json!({
            "error": error,
            "safe_message": safe_message
        }),
    )
}

fn not_implemented(error: &'static str, safe_message: &'static str) -> FacadeResponse {
    json_facade_response(
        StatusCode::NOT_IMPLEMENTED,
        json!({
            "error": error,
            "safe_message": safe_message
        }),
    )
}

fn unauthorized() -> FacadeResponse {
    json_facade_response(
        StatusCode::UNAUTHORIZED,
        json!({
            "error": "missing_oar_session",
            "safe_message": "A valid OAR session bearer token is required."
        }),
    )
}

fn has_oar_session(authorization: Option<&str>) -> bool {
    authorization
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(|session_id| !session_id.trim().is_empty())
        .unwrap_or(false)
}

fn is_auth_session_status_route(method: &Method, path: &str) -> bool {
    *method == Method::GET
        && path
            .strip_prefix("/auth/feishu/qr-sessions/")
            .is_some_and(|session_id| !session_id.is_empty() && !session_id.ends_with("/events"))
}

fn is_auth_session_events_route(method: &Method, path: &str) -> bool {
    *method == Method::GET
        && path
            .strip_prefix("/auth/feishu/qr-sessions/")
            .is_some_and(|session_id| {
                session_id
                    .strip_suffix("/events")
                    .is_some_and(|id| !id.is_empty())
            })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthz_returns_safe_service_status() {
        let response = dispatch_request(&Method::GET, "/healthz", None, None);
        let body: Value = serde_json::from_str(&response.body).expect("json");

        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(body["status"], "ok");
        assert!(!response.body.contains("token"));
    }

    #[test]
    fn config_defaults_to_localhost_and_accepts_docker_bind_override() {
        let default_config = OarHttpFacadeConfig::from_env_map(&|_| None).expect("default config");
        let docker_config = OarHttpFacadeConfig::from_env_map(&|key| {
            (key == "OAR_HTTP_BIND_ADDR").then(|| "0.0.0.0:8080".to_string())
        })
        .expect("docker config");

        assert_eq!(
            default_config.bind_addr,
            "127.0.0.1:8080".parse::<SocketAddr>().expect("addr")
        );
        assert_eq!(
            docker_config.bind_addr,
            "0.0.0.0:8080".parse::<SocketAddr>().expect("addr")
        );
    }

    #[test]
    fn config_rejects_invalid_bind_override_without_echoing_in_display() {
        let error = OarHttpFacadeConfig::from_env_map(&|key| {
            (key == "OAR_HTTP_BIND_ADDR").then(|| "not an address".to_string())
        })
        .expect_err("invalid config");

        assert_eq!(
            error.to_string(),
            "oar_http_facade_config_invalid: invalid_bind_addr"
        );
        assert!(!error.to_string().contains("not an address"));
    }

    #[test]
    fn snapshot_requires_oar_session_and_returns_empty_contract() {
        let unauthorized = dispatch_request(&Method::GET, "/review-inbox/snapshot", None, None);
        assert_eq!(unauthorized.status, StatusCode::UNAUTHORIZED);

        let response = dispatch_request(
            &Method::GET,
            "/review-inbox/snapshot",
            Some("Bearer oar_session_dev"),
            Some("application/json"),
        );
        let body: Value = serde_json::from_str(&response.body).expect("json");

        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(body["contract_version"], 1);
        assert!(body["items"].as_array().expect("items").is_empty());
        assert!(body["proposed_actions"]
            .as_array()
            .expect("proposed_actions")
            .is_empty());
    }

    #[test]
    fn decisions_are_rejected_until_ledger_path_is_wired() {
        let response = dispatch_request(
            &Method::POST,
            "/review-inbox/decisions",
            Some("Bearer oar_session_dev"),
            Some("application/json"),
        );
        let body: Value = serde_json::from_str(&response.body).expect("json");

        assert_eq!(response.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["error"], "review_decision_not_wired");
    }

    #[test]
    fn auth_routes_do_not_fake_real_feishu_login() {
        let create = dispatch_request(&Method::POST, "/auth/feishu/qr-sessions", None, None);
        let poll = dispatch_request(&Method::GET, "/auth/feishu/qr-sessions/qr_dev", None, None);
        let events = dispatch_request(
            &Method::GET,
            "/auth/feishu/qr-sessions/qr_dev/events",
            None,
            Some("text/event-stream"),
        );

        assert_eq!(create.status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(poll.status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(events.status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(!create.body.contains("access_token"));
        assert!(!poll.body.contains("refresh_token"));
        assert!(!events.body.contains("authorization"));
    }
}

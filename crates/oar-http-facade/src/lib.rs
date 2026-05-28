#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::convert::Infallible;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::header::{ACCEPT, AUTHORIZATION, CACHE_CONTROL, CONTENT_TYPE};
use hyper::http::{HeaderValue, Method, StatusCode};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use oar_lark_adapter::{
    AsyncFeishuOAuthLogin, FeishuOAuthLoginClient, FeishuOAuthLoginConfig, FeishuOpenApiConfig,
    ReqwestAsyncHttpClient,
};
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

#[derive(Clone, Default)]
pub struct OarHttpFacadeRuntime {
    feishu_login: Option<Arc<FeishuLoginRuntime>>,
}

impl fmt::Debug for OarHttpFacadeRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OarHttpFacadeRuntime")
            .field("feishu_login", &self.feishu_login.is_some())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OarHttpFacadeRuntimeError {
    PartialFeishuAuthConfig,
    InvalidFeishuOpenApiConfig,
    InvalidFeishuLoginConfig,
    HttpClientBuildFailed,
}

impl fmt::Display for OarHttpFacadeRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PartialFeishuAuthConfig => {
                write!(f, "oar_feishu_auth_config_partial")
            }
            Self::InvalidFeishuOpenApiConfig => {
                write!(f, "oar_feishu_open_api_config_invalid")
            }
            Self::InvalidFeishuLoginConfig => {
                write!(f, "oar_feishu_login_config_invalid")
            }
            Self::HttpClientBuildFailed => write!(f, "oar_feishu_http_client_build_failed"),
        }
    }
}

impl Error for OarHttpFacadeRuntimeError {}

impl OarHttpFacadeRuntime {
    pub fn disabled() -> Self {
        Self::default()
    }

    pub fn from_env_map(
        env: &impl Fn(&str) -> Option<String>,
    ) -> Result<Self, OarHttpFacadeRuntimeError> {
        let app_id = non_empty_env(env, "OAR_FEISHU_APP_ID");
        let app_secret = non_empty_env(env, "OAR_FEISHU_APP_SECRET");
        let redirect_uri = non_empty_env(env, "OAR_FEISHU_REDIRECT_URI");
        let has_any_auth_config =
            app_id.is_some() || app_secret.is_some() || redirect_uri.is_some();
        if !has_any_auth_config {
            return Ok(Self::disabled());
        }

        let (Some(app_id), Some(app_secret), Some(redirect_uri)) =
            (app_id, app_secret, redirect_uri)
        else {
            return Err(OarHttpFacadeRuntimeError::PartialFeishuAuthConfig);
        };

        let open_api = FeishuOpenApiConfig::from_env_map(env)
            .map_err(|_| OarHttpFacadeRuntimeError::InvalidFeishuOpenApiConfig)?;
        let authorize_base_url = non_empty_env(env, "OAR_FEISHU_AUTHORIZE_BASE_URL")
            .unwrap_or_else(|| "https://open.feishu.cn".to_string());
        let scope = non_empty_env(env, "OAR_FEISHU_AUTH_SCOPE")
            .or_else(|| Some("auth:user.id:read offline_access".to_string()));
        let login_config = FeishuOAuthLoginConfig::new(
            open_api.clone(),
            authorize_base_url,
            app_id,
            app_secret,
            redirect_uri,
            scope,
        )
        .map_err(|_| OarHttpFacadeRuntimeError::InvalidFeishuLoginConfig)?;
        let http_client = ReqwestAsyncHttpClient::with_config(&open_api)
            .map_err(|_| OarHttpFacadeRuntimeError::HttpClientBuildFailed)?;
        Ok(Self {
            feishu_login: Some(Arc::new(FeishuLoginRuntime {
                config: login_config,
                http_client,
                sessions: Mutex::new(HashMap::new()),
            })),
        })
    }
}

struct FeishuLoginRuntime {
    config: FeishuOAuthLoginConfig,
    http_client: ReqwestAsyncHttpClient,
    sessions: Mutex<HashMap<String, FeishuLoginSession>>,
}

impl fmt::Debug for FeishuLoginRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuLoginRuntime")
            .field("config", &self.config)
            .field("http_client", &"[REDACTED]")
            .field("sessions", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone)]
struct FeishuLoginSession {
    id: String,
    qr_page_url: String,
    expires_at: SystemTime,
    state: FeishuLoginSessionState,
}

#[derive(Debug, Clone)]
enum FeishuLoginSessionState {
    Pending,
    Authorized {
        oar_session_id: String,
        user_id: String,
        display_name: String,
        tenant_name: String,
    },
    Denied {
        safe_message: String,
    },
    Expired,
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
    run_with_runtime(config, OarHttpFacadeRuntime::disabled()).await
}

pub async fn run_with_runtime(
    config: OarHttpFacadeConfig,
    runtime: OarHttpFacadeRuntime,
) -> Result<(), OarHttpFacadeError> {
    let listener = TcpListener::bind(config.bind_addr)
        .await
        .map_err(OarHttpFacadeError::Bind)?;
    info!(bind_addr = %config.bind_addr, "oar http facade listening");
    let runtime = Arc::new(runtime);

    loop {
        let (stream, remote_addr) = listener
            .accept()
            .await
            .map_err(OarHttpFacadeError::Accept)?;
        let runtime = Arc::clone(&runtime);
        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            if let Err(error) = http1::Builder::new()
                .serve_connection(
                    io,
                    service_fn(move |request| {
                        handle_hyper_request_with_runtime(Arc::clone(&runtime), request)
                    }),
                )
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
    handle_hyper_request_with_runtime(Arc::new(OarHttpFacadeRuntime::disabled()), request).await
}

pub async fn handle_hyper_request_with_runtime(
    runtime: Arc<OarHttpFacadeRuntime>,
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
    let facade_response = dispatch_request_with_runtime(
        runtime,
        request.method(),
        request.uri().path(),
        request.uri().query(),
        authorization,
        accept,
    )
    .await;
    Ok(facade_response.into_hyper_response())
}

pub async fn dispatch_request_with_runtime(
    runtime: Arc<OarHttpFacadeRuntime>,
    method: &Method,
    path: &str,
    query: Option<&str>,
    authorization: Option<&str>,
    accept: Option<&str>,
) -> FacadeResponse {
    match (method, path) {
        (&Method::POST, "/auth/feishu/qr-sessions") => {
            return create_feishu_login_session(runtime.feishu_login.as_deref());
        }
        (&Method::GET, "/auth/feishu/callback") => {
            return complete_feishu_login_callback(runtime.feishu_login.as_deref(), query).await;
        }
        _ if is_auth_session_status_route(method, path) => {
            let Some(session_id) = auth_session_status_id(path) else {
                return not_found();
            };
            return feishu_login_session_status(runtime.feishu_login.as_deref(), session_id);
        }
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
            let Some(session_id) = auth_session_events_id(path) else {
                return not_found();
            };
            return feishu_login_session_event(runtime.feishu_login.as_deref(), session_id);
        }
        _ => {}
    }

    dispatch_request(method, path, authorization, accept)
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

fn html_facade_response(status: StatusCode, body: String) -> FacadeResponse {
    FacadeResponse {
        status,
        content_type: "text/html; charset=utf-8",
        body,
    }
}

fn sse_facade_response(body: String) -> FacadeResponse {
    FacadeResponse {
        status: StatusCode::OK,
        content_type: "text/event-stream",
        body,
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

fn callback_html(status: StatusCode, title: &str, message: &str) -> FacadeResponse {
    html_facade_response(
        status,
        format!(
            r#"<!doctype html><html lang="zh-CN"><meta charset="utf-8"><title>{}</title><body style="font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;margin:0;display:grid;place-items:center;min-height:100vh;background:#f6f7f8;color:#202124"><main style="max-width:520px;padding:32px"><h1 style="font-size:22px;margin:0 0 12px">{}</h1><p style="font-size:15px;line-height:1.6;margin:0;color:#5f6368">{}</p></main></body></html>"#,
            escape_html(title),
            escape_html(title),
            escape_html(message)
        ),
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

fn not_found() -> FacadeResponse {
    json_facade_response(
        StatusCode::NOT_FOUND,
        json!({
            "error": "not_found",
            "safe_message": "No OAR backend route matched this request."
        }),
    )
}

fn has_oar_session(authorization: Option<&str>) -> bool {
    authorization
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(|session_id| !session_id.trim().is_empty())
        .unwrap_or(false)
}

fn create_feishu_login_session(runtime: Option<&FeishuLoginRuntime>) -> FacadeResponse {
    let Some(runtime) = runtime else {
        return service_unavailable(
            "feishu_auth_not_configured",
            "Feishu QR login is not configured in this backend facade.",
        );
    };

    let Ok(session_id) = secure_random_hex(18) else {
        return service_unavailable(
            "feishu_auth_session_unavailable",
            "Feishu QR login session could not be created.",
        );
    };
    let expires_at = SystemTime::now() + Duration::from_secs(300);
    let qr_page_url = runtime.config.authorization_url(&session_id);
    let session = FeishuLoginSession {
        id: session_id.clone(),
        qr_page_url,
        expires_at,
        state: FeishuLoginSessionState::Pending,
    };
    let body = qr_session_json(&session);
    runtime
        .sessions
        .lock()
        .expect("feishu login session mutex")
        .insert(session_id, session);
    json_facade_response(StatusCode::CREATED, body)
}

fn feishu_login_session_status(
    runtime: Option<&FeishuLoginRuntime>,
    session_id: &str,
) -> FacadeResponse {
    let Some(runtime) = runtime else {
        return service_unavailable(
            "feishu_auth_not_configured",
            "Feishu QR login is not configured in this backend facade.",
        );
    };
    let mut sessions = runtime.sessions.lock().expect("feishu login session mutex");
    let Some(session) = sessions.get_mut(session_id) else {
        return json_facade_response(
            StatusCode::NOT_FOUND,
            json!({
                "error": "feishu_auth_session_not_found",
                "safe_message": "Feishu login session was not found."
            }),
        );
    };
    expire_session_if_needed(session);
    json_facade_response(StatusCode::OK, session_status_json(session))
}

fn feishu_login_session_event(
    runtime: Option<&FeishuLoginRuntime>,
    session_id: &str,
) -> FacadeResponse {
    let response = feishu_login_session_status(runtime, session_id);
    if response.status != StatusCode::OK {
        return response;
    }
    let Ok(status) = serde_json::from_str::<Value>(&response.body) else {
        return service_unavailable(
            "feishu_auth_event_unavailable",
            "Feishu QR login event could not be created.",
        );
    };
    let event = match status.get("status").and_then(Value::as_str) {
        Some("pending") => "pending",
        Some("authorized") => "authorized",
        Some("denied") => "denied",
        Some("expired") => "expired",
        _ => "keepalive",
    };
    sse_facade_response(format!(
        "event: {event}\ndata: {}\n\n",
        auth_event_json(session_id, event, &status)
    ))
}

async fn complete_feishu_login_callback(
    runtime: Option<&FeishuLoginRuntime>,
    query: Option<&str>,
) -> FacadeResponse {
    let Some(runtime) = runtime else {
        return service_unavailable(
            "feishu_auth_not_configured",
            "Feishu QR login is not configured in this backend facade.",
        );
    };
    let params = parse_query(query.unwrap_or_default());
    let Some(state) = params.get("state").filter(|value| !value.trim().is_empty()) else {
        return callback_html(
            StatusCode::BAD_REQUEST,
            "飞书登录失败",
            "缺少登录状态，请回到 OAR 重新发起扫码登录。",
        );
    };
    if params
        .get("error")
        .is_some_and(|error| error == "access_denied")
    {
        mark_session_denied(runtime, state, "飞书扫码授权已取消。");
        return callback_html(StatusCode::OK, "授权已取消", "可以回到 OAR 重新发起登录。");
    }
    let Some(code) = params.get("code").filter(|value| !value.trim().is_empty()) else {
        mark_session_denied(runtime, state, "飞书没有返回授权码。");
        return callback_html(
            StatusCode::BAD_REQUEST,
            "飞书登录失败",
            "飞书没有返回授权码，请回到 OAR 重新发起登录。",
        );
    };

    {
        let mut sessions = runtime.sessions.lock().expect("feishu login session mutex");
        let Some(session) = sessions.get_mut(state) else {
            return callback_html(
                StatusCode::NOT_FOUND,
                "登录会话不存在",
                "这次登录会话不存在或已过期，请回到 OAR 重新发起登录。",
            );
        };
        expire_session_if_needed(session);
        if matches!(
            session.state,
            FeishuLoginSessionState::Denied { .. } | FeishuLoginSessionState::Expired
        ) {
            return callback_html(
                StatusCode::GONE,
                "登录会话已失效",
                "这次登录会话已失效，请回到 OAR 重新发起登录。",
            );
        }
    }

    let mut client =
        FeishuOAuthLoginClient::new(runtime.config.clone(), runtime.http_client.clone());
    let login = match client.exchange_code(code).await {
        Ok(login) => login,
        Err(error) => {
            tracing::warn!(?error, "feishu oauth login callback failed");
            mark_session_denied(runtime, state, "飞书授权验证失败，请重新扫码。");
            return callback_html(
                StatusCode::BAD_GATEWAY,
                "飞书登录失败",
                "飞书授权验证失败，请回到 OAR 重新发起登录。",
            );
        }
    };

    let oar_session_id = format!(
        "oar_session_{}",
        secure_random_hex(18).unwrap_or_else(|_| sanitize_session_suffix(state))
    );
    let tenant_name = login
        .user
        .tenant_key
        .clone()
        .unwrap_or_else(|| "Feishu".to_string());
    {
        let mut sessions = runtime.sessions.lock().expect("feishu login session mutex");
        let Some(session) = sessions.get_mut(state) else {
            return callback_html(
                StatusCode::NOT_FOUND,
                "登录会话不存在",
                "这次登录会话不存在或已过期，请回到 OAR 重新发起登录。",
            );
        };
        session.state = FeishuLoginSessionState::Authorized {
            oar_session_id,
            user_id: login.user.open_id,
            display_name: login.user.display_name,
            tenant_name,
        };
    }

    callback_html(StatusCode::OK, "OAR 登录成功", "可以回到 OAR 客户端继续。")
}

fn mark_session_denied(runtime: &FeishuLoginRuntime, session_id: &str, safe_message: &str) {
    if let Some(session) = runtime
        .sessions
        .lock()
        .expect("feishu login session mutex")
        .get_mut(session_id)
    {
        session.state = FeishuLoginSessionState::Denied {
            safe_message: safe_message.to_string(),
        };
    }
}

fn expire_session_if_needed(session: &mut FeishuLoginSession) {
    if matches!(session.state, FeishuLoginSessionState::Pending)
        && SystemTime::now() > session.expires_at
    {
        session.state = FeishuLoginSessionState::Expired;
    }
}

fn qr_session_json(session: &FeishuLoginSession) -> Value {
    json!({
        "session_id": session.id,
        "qr_page_url": session.qr_page_url,
        "expires_at": iso8601_utc(session.expires_at)
    })
}

fn session_status_json(session: &FeishuLoginSession) -> Value {
    match &session.state {
        FeishuLoginSessionState::Pending => json!({
            "status": "pending",
            "qr_session": qr_session_json(session),
            "oar_session": null,
            "user": null,
            "safe_message": null
        }),
        FeishuLoginSessionState::Authorized {
            oar_session_id,
            user_id,
            display_name,
            tenant_name,
        } => json!({
            "status": "authorized",
            "qr_session": null,
            "oar_session": {
                "session_id": oar_session_id
            },
            "user": {
                "id": user_id,
                "display_name": display_name,
                "tenant_name": tenant_name
            },
            "safe_message": null
        }),
        FeishuLoginSessionState::Denied { safe_message } => json!({
            "status": "denied",
            "qr_session": null,
            "oar_session": null,
            "user": null,
            "safe_message": safe_message
        }),
        FeishuLoginSessionState::Expired => json!({
            "status": "expired",
            "qr_session": null,
            "oar_session": null,
            "user": null,
            "safe_message": "飞书登录二维码已过期。"
        }),
    }
}

fn auth_event_json(session_id: &str, event: &str, status: &Value) -> Value {
    json!({
        "event": event,
        "session_id": session_id,
        "qr_session": status.get("qr_session").cloned().unwrap_or(Value::Null),
        "oar_session": status.get("oar_session").cloned().unwrap_or(Value::Null),
        "user": status.get("user").cloned().unwrap_or(Value::Null),
        "safe_message": status.get("safe_message").cloned().unwrap_or(Value::Null),
        "event_id": format!("auth_evt_{}_{}", session_id, event)
    })
}

fn auth_session_status_id(path: &str) -> Option<&str> {
    path.strip_prefix("/auth/feishu/qr-sessions/")
        .filter(|session_id| !session_id.is_empty() && !session_id.ends_with("/events"))
}

fn auth_session_events_id(path: &str) -> Option<&str> {
    path.strip_prefix("/auth/feishu/qr-sessions/")
        .and_then(|session_id| session_id.strip_suffix("/events"))
        .filter(|session_id| !session_id.is_empty())
}

fn is_auth_session_status_route(method: &Method, path: &str) -> bool {
    *method == Method::GET && auth_session_status_id(path).is_some()
}

fn is_auth_session_events_route(method: &Method, path: &str) -> bool {
    *method == Method::GET && auth_session_events_id(path).is_some()
}

fn non_empty_env(env: &impl Fn(&str) -> Option<String>, key: &str) -> Option<String> {
    env(key).and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn secure_random_hex(bytes_len: usize) -> std::io::Result<String> {
    let mut bytes = vec![0_u8; bytes_len];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>())
}

fn sanitize_session_suffix(value: &str) -> String {
    let sanitized = value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>();
    if sanitized.is_empty() {
        "fallback".to_string()
    } else {
        sanitized
    }
}

fn parse_query(query: &str) -> HashMap<String, String> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .filter_map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            Some((percent_decode(key)?, percent_decode(value)?))
        })
        .collect()
}

fn percent_decode(value: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(value.len());
    let raw = value.as_bytes();
    let mut index = 0;
    while index < raw.len() {
        match raw[index] {
            b'+' => {
                bytes.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < raw.len() => {
                let hex = std::str::from_utf8(&raw[index + 1..index + 3]).ok()?;
                let decoded = u8::from_str_radix(hex, 16).ok()?;
                bytes.push(decoded);
                index += 3;
            }
            b'%' => return None,
            byte => {
                bytes.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(bytes).ok()
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn iso8601_utc(time: SystemTime) -> String {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs() as i64;
    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i64, u32, u32) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    (year, month as u32, day as u32)
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
    fn runtime_disables_auth_when_env_absent_and_rejects_partial_auth_config() {
        let disabled = OarHttpFacadeRuntime::from_env_map(&|_| None).expect("disabled runtime");
        assert!(disabled.feishu_login.is_none());

        let partial = OarHttpFacadeRuntime::from_env_map(&|key| {
            (key == "OAR_FEISHU_APP_ID").then(|| "cli_test".to_string())
        })
        .expect_err("partial auth config");

        assert_eq!(
            partial.to_string(),
            "oar_feishu_auth_config_partial".to_string()
        );
        assert!(!format!("{partial:?}").contains("cli_test"));
    }

    #[test]
    fn configured_runtime_creates_pending_feishu_login_session_without_leaking_secret() {
        let runtime = OarHttpFacadeRuntime::from_env_map(&|key| match key {
            "OAR_FEISHU_APP_ID" => Some("cli_test".to_string()),
            "OAR_FEISHU_APP_SECRET" => Some("super-secret".to_string()),
            "OAR_FEISHU_REDIRECT_URI" => {
                Some("https://oar.example.test/auth/feishu/callback".to_string())
            }
            _ => None,
        })
        .expect("runtime");

        let response = create_feishu_login_session(runtime.feishu_login.as_deref());
        let body: Value = serde_json::from_str(&response.body).expect("json");

        assert_eq!(response.status, StatusCode::CREATED);
        assert!(body["session_id"].as_str().expect("session id").len() >= 32);
        assert!(body["qr_page_url"]
            .as_str()
            .expect("qr url")
            .contains("/open-apis/authen/v1/authorize"));
        assert!(body["qr_page_url"]
            .as_str()
            .expect("qr url")
            .contains("client_id=cli_test"));
        assert!(!response.body.contains("super-secret"));
    }

    #[tokio::test]
    async fn configured_runtime_dispatch_creates_and_polls_pending_feishu_login_session() {
        let runtime = Arc::new(
            OarHttpFacadeRuntime::from_env_map(&|key| match key {
                "OAR_FEISHU_APP_ID" => Some("cli_test".to_string()),
                "OAR_FEISHU_APP_SECRET" => Some("super-secret".to_string()),
                "OAR_FEISHU_REDIRECT_URI" => {
                    Some("https://oar.example.test/auth/feishu/callback".to_string())
                }
                _ => None,
            })
            .expect("runtime"),
        );

        let create = dispatch_request_with_runtime(
            Arc::clone(&runtime),
            &Method::POST,
            "/auth/feishu/qr-sessions",
            None,
            None,
            None,
        )
        .await;
        let created: Value = serde_json::from_str(&create.body).expect("create json");
        let session_path = format!(
            "/auth/feishu/qr-sessions/{}",
            created["session_id"].as_str().expect("session id")
        );
        let poll =
            dispatch_request_with_runtime(runtime, &Method::GET, &session_path, None, None, None)
                .await;
        let status: Value = serde_json::from_str(&poll.body).expect("poll json");

        assert_eq!(create.status, StatusCode::CREATED);
        assert_eq!(poll.status, StatusCode::OK);
        assert_eq!(status["status"], "pending");
        assert_eq!(status["qr_session"]["session_id"], created["session_id"]);
        assert!(!create.body.contains("super-secret"));
        assert!(!poll.body.contains("super-secret"));
    }

    #[test]
    fn iso8601_formatter_uses_utc_epoch_contract() {
        assert_eq!(iso8601_utc(UNIX_EPOCH), "1970-01-01T00:00:00Z");
        assert_eq!(
            iso8601_utc(UNIX_EPOCH + Duration::from_secs(86_400)),
            "1970-01-02T00:00:00Z"
        );
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

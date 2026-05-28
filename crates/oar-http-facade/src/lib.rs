#![forbid(unsafe_code)]

use std::collections::{BTreeSet, HashMap};
use std::convert::Infallible;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full, StreamBody};
use hyper::body::{Frame, Incoming};
use hyper::header::{ACCEPT, AUTHORIZATION, CACHE_CONTROL, CONTENT_TYPE};
use hyper::http::{HeaderValue, Method, StatusCode};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use oar_core::domain::device_sync::{DeviceEntryPoint, DeviceSession, SessionState};
use oar_core::domain::identity::{
    ActorKind, DeviceSessionId, LarkIdentity, LarkIdentityId, ScopeBoundary, Tenant, TenantId,
    TenantStatus, TokenGrantState, WorkspaceUser, WorkspaceUserId, WorkspaceUserStatus,
};
use oar_core::storage::postgres::{
    EncryptedTokenGrantRecord, PostgresDeviceSessionRepository, PostgresLarkIdentityRepository,
    PostgresTenantRepository, PostgresTokenGrantRepository, PostgresWorkspaceUserRepository,
    StoredDeviceSession,
};
use oar_lark_adapter::material::compose_encrypted_grant_blob;
use oar_lark_adapter::{
    AesGcmGrantEncryptor, AsyncFeishuOAuthLogin, FeishuGrantEncryptionInput, FeishuGrantEncryptor,
    FeishuOAuthLogin, FeishuOAuthLoginClient, FeishuOAuthLoginConfig, FeishuOpenApiConfig,
    PostgresFeishuAuthRefreshEnvConfig, ReqwestAsyncHttpClient,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Notify};
use tokio::time;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{error, info};

type ResponseBody = BoxBody<Bytes, Infallible>;
const FEISHU_LOGIN_SSE_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);

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
    InvalidFeishuGrantConfig,
    DatabaseConnectFailed,
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
            Self::InvalidFeishuGrantConfig => {
                write!(f, "oar_feishu_grant_config_invalid")
            }
            Self::DatabaseConnectFailed => write!(f, "oar_database_connect_failed"),
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
        Self::from_env_map_with_persistence(env, None)
    }

    pub async fn from_env_map_async(
        env: &impl Fn(&str) -> Option<String>,
    ) -> Result<Self, OarHttpFacadeRuntimeError> {
        let runtime = Self::from_env_map(env)?;
        if runtime.feishu_login.is_none() {
            return Ok(runtime);
        }

        let Some(database_url) = non_empty_env(env, "DATABASE_URL") else {
            return Ok(runtime);
        };
        let grant_config = PostgresFeishuAuthRefreshEnvConfig::from_env_map(env)
            .map_err(|_| OarHttpFacadeRuntimeError::InvalidFeishuGrantConfig)?;
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
            .map_err(|_| OarHttpFacadeRuntimeError::DatabaseConnectFailed)?;
        Self::from_env_map_with_persistence(
            env,
            Some(FeishuGrantPersistenceRuntime {
                pool,
                grant_key_id: grant_config.grant_key_id,
                grant_key_material: grant_config.grant_key_material,
            }),
        )
    }

    fn from_env_map_with_persistence(
        env: &impl Fn(&str) -> Option<String>,
        grant_persistence: Option<FeishuGrantPersistenceRuntime>,
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
            .or_else(|| Some("offline_access".to_string()));
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
                grant_persistence,
                sessions: Mutex::new(HashMap::new()),
            })),
        })
    }
}

struct FeishuLoginRuntime {
    config: FeishuOAuthLoginConfig,
    http_client: ReqwestAsyncHttpClient,
    grant_persistence: Option<FeishuGrantPersistenceRuntime>,
    sessions: Mutex<HashMap<String, FeishuLoginSession>>,
}

impl fmt::Debug for FeishuLoginRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuLoginRuntime")
            .field("config", &self.config)
            .field("http_client", &"[REDACTED]")
            .field("grant_persistence", &self.grant_persistence.is_some())
            .field("sessions", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone)]
struct FeishuGrantPersistenceRuntime {
    pool: PgPool,
    grant_key_id: String,
    grant_key_material: [u8; 32],
}

impl fmt::Debug for FeishuGrantPersistenceRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuGrantPersistenceRuntime")
            .field("pool", &"[REDACTED]")
            .field("grant_key_id", &"[REDACTED]")
            .field("grant_key_material", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone)]
struct FeishuLoginSession {
    id: String,
    qr_page_url: String,
    expires_at: SystemTime,
    state: FeishuLoginSessionState,
    event_version: u64,
    notify: Arc<Notify>,
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

#[derive(Debug, Clone)]
struct FeishuLoginPersistencePlan {
    tenant: Tenant,
    user: WorkspaceUser,
    identity: LarkIdentity,
    grant: EncryptedTokenGrantRecord,
    session: DeviceSession,
    session_identity_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FeishuLoginPersistenceError {
    MissingTenantKey,
    MissingRefreshToken,
    EncryptGrantFailed,
    StoreFailed { stage: &'static str },
}

impl fmt::Display for FeishuLoginPersistenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingTenantKey => write!(f, "feishu_login_missing_tenant_key"),
            Self::MissingRefreshToken => write!(f, "feishu_login_missing_refresh_token"),
            Self::EncryptGrantFailed => write!(f, "feishu_login_grant_encrypt_failed"),
            Self::StoreFailed { stage } => {
                write!(f, "feishu_login_grant_store_failed:{stage}")
            }
        }
    }
}

impl Error for FeishuLoginPersistenceError {}

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

#[derive(Debug, Clone, PartialEq, Eq)]
struct AuthenticatedContext {
    session_id: String,
    tenant_id: String,
    user_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum OarSessionAuthError {
    MissingBearer,
    InvalidSession,
    StoreUnavailable,
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
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let query = request.uri().query().map(str::to_string);
    let authorization = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok());
    let accept = request
        .headers()
        .get(ACCEPT)
        .and_then(|value| value.to_str().ok());
    if is_auth_session_events_route(&method, &path) {
        if !accepts_event_stream(accept) {
            return Ok(json_facade_response(
                StatusCode::NOT_ACCEPTABLE,
                json!({
                    "error": "event_stream_required",
                    "safe_message": "Auth session events require Accept: text/event-stream."
                }),
            )
            .into_hyper_response());
        }
        let Some(session_id) = auth_session_events_id(&path) else {
            return Ok(not_found().into_hyper_response());
        };
        return Ok(feishu_login_session_event_stream_response(
            runtime.feishu_login.clone(),
            session_id.to_string(),
        ));
    }

    let facade_response = dispatch_request_with_runtime(
        runtime,
        &method,
        &path,
        query.as_deref(),
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
            if !accepts_event_stream(accept) {
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
        (&Method::GET, "/review-inbox/snapshot") => {
            let auth_context = match authenticate_oar_session(&runtime, authorization).await {
                Ok(context) => context,
                Err(error) => return oar_session_auth_error_response(error),
            };
            return review_inbox_snapshot_for_context(&auth_context);
        }
        (&Method::POST, "/review-inbox/decisions") => {
            let auth_context = match authenticate_oar_session(&runtime, authorization).await {
                Ok(context) => context,
                Err(error) => return oar_session_auth_error_response(error),
            };
            return review_decision_not_wired_for_context(&auth_context);
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
        (&Method::GET, "/review-inbox/snapshot") => protected_route_requires_session_store(
            authorization,
            "Review inbox requires verified OAR session storage.",
        ),
        (&Method::POST, "/review-inbox/decisions") => protected_route_requires_session_store(
            authorization,
            "Review decisions require verified OAR session storage.",
        ),
        _ if is_auth_session_status_route(method, path) => service_unavailable(
            "feishu_auth_not_configured",
            "Feishu QR login is not configured in this backend facade.",
        ),
        _ if is_auth_session_events_route(method, path) => {
            if !accepts_event_stream(accept) {
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
        let mut response = Response::new(full_response_body(self.body));
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

fn full_response_body(body: String) -> ResponseBody {
    Full::new(Bytes::from(body))
        .map_err(|never| match never {})
        .boxed()
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

fn invalid_oar_session() -> FacadeResponse {
    json_facade_response(
        StatusCode::UNAUTHORIZED,
        json!({
            "error": "invalid_oar_session",
            "safe_message": "The OAR session is invalid or expired."
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

fn protected_route_requires_session_store(
    authorization: Option<&str>,
    safe_message: &'static str,
) -> FacadeResponse {
    match bearer_session_id(authorization) {
        Ok(_) => service_unavailable("oar_session_verification_unavailable", safe_message),
        Err(error) => oar_session_auth_error_response(error),
    }
}

async fn authenticate_oar_session(
    runtime: &OarHttpFacadeRuntime,
    authorization: Option<&str>,
) -> Result<AuthenticatedContext, OarSessionAuthError> {
    let session_id = bearer_session_id(authorization)?;
    let persistence = runtime
        .feishu_login
        .as_ref()
        .and_then(|login| login.grant_persistence.as_ref())
        .ok_or(OarSessionAuthError::StoreUnavailable)?;
    let session = PostgresDeviceSessionRepository::new(persistence.pool.clone())
        .get_by_session_id_for_authentication(session_id)
        .await
        .map_err(|_| OarSessionAuthError::StoreUnavailable)?
        .ok_or(OarSessionAuthError::InvalidSession)?;
    authenticated_context_from_session(&session)
}

fn authenticated_context_from_session(
    session: &StoredDeviceSession,
) -> Result<AuthenticatedContext, OarSessionAuthError> {
    if session.state != SessionState::Active
        || session.revoked_at.is_some()
        || session.expired_at.is_some()
    {
        return Err(OarSessionAuthError::InvalidSession);
    }
    Ok(AuthenticatedContext {
        session_id: session.id.clone(),
        tenant_id: session.tenant_id.clone(),
        user_id: session.user_id.clone(),
    })
}

fn bearer_session_id(authorization: Option<&str>) -> Result<&str, OarSessionAuthError> {
    let session_id = authorization
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(OarSessionAuthError::MissingBearer)?;
    if !session_id.starts_with("oar_session_") {
        return Err(OarSessionAuthError::InvalidSession);
    }
    Ok(session_id)
}

fn oar_session_auth_error_response(error: OarSessionAuthError) -> FacadeResponse {
    match error {
        OarSessionAuthError::MissingBearer => unauthorized(),
        OarSessionAuthError::InvalidSession => invalid_oar_session(),
        OarSessionAuthError::StoreUnavailable => service_unavailable(
            "oar_session_verification_unavailable",
            "OAR session verification is temporarily unavailable.",
        ),
    }
}

fn review_inbox_snapshot_for_context(context: &AuthenticatedContext) -> FacadeResponse {
    let _ = (&context.session_id, &context.tenant_id, &context.user_id);
    json_facade_response(StatusCode::OK, empty_review_inbox_snapshot())
}

fn review_decision_not_wired_for_context(context: &AuthenticatedContext) -> FacadeResponse {
    let _ = (&context.session_id, &context.tenant_id, &context.user_id);
    json_facade_response(
        StatusCode::UNPROCESSABLE_ENTITY,
        json!({
            "error": "review_decision_not_wired",
            "safe_message": "Review decisions are disabled until the ConfirmedAction ledger path is connected."
        }),
    )
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
        event_version: 0,
        notify: Arc::new(Notify::new()),
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
    let Some(runtime) = runtime else {
        return service_unavailable(
            "feishu_auth_not_configured",
            "Feishu QR login is not configured in this backend facade.",
        );
    };
    match feishu_login_event_snapshot(runtime, session_id) {
        Ok(snapshot) => sse_facade_response(snapshot.frame),
        Err(response) => response,
    }
}

fn feishu_login_session_event_stream_response(
    runtime: Option<Arc<FeishuLoginRuntime>>,
    session_id: String,
) -> Response<ResponseBody> {
    let Some(runtime) = runtime else {
        return service_unavailable(
            "feishu_auth_not_configured",
            "Feishu QR login is not configured in this backend facade.",
        )
        .into_hyper_response();
    };
    let initial_snapshot = match feishu_login_event_snapshot(&runtime, &session_id) {
        Ok(snapshot) => snapshot,
        Err(response) => return response.into_hyper_response(),
    };

    let (sender, receiver) = mpsc::channel::<Result<Frame<Bytes>, Infallible>>(8);
    tokio::spawn(async move {
        if send_sse_frame(&sender, initial_snapshot.frame)
            .await
            .is_err()
            || initial_snapshot.is_terminal
        {
            return;
        }

        let notify = initial_snapshot.notify;
        let mut last_version = initial_snapshot.version;
        let mut keepalive = time::interval(FEISHU_LOGIN_SSE_KEEPALIVE_INTERVAL);
        keepalive.tick().await;

        loop {
            let notified = notify.notified();
            tokio::pin!(notified);

            match feishu_login_event_snapshot(&runtime, &session_id) {
                Ok(snapshot) if snapshot.version != last_version => {
                    last_version = snapshot.version;
                    let is_terminal = snapshot.is_terminal;
                    if send_sse_frame(&sender, snapshot.frame).await.is_err() || is_terminal {
                        break;
                    }
                    continue;
                }
                Ok(_) => {}
                Err(_) => break,
            }

            tokio::select! {
                _ = &mut notified => {
                    match feishu_login_event_snapshot(&runtime, &session_id) {
                        Ok(snapshot) => {
                            last_version = snapshot.version;
                            let is_terminal = snapshot.is_terminal;
                            if send_sse_frame(&sender, snapshot.frame).await.is_err() || is_terminal {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                _ = keepalive.tick() => {
                    match feishu_login_event_snapshot(&runtime, &session_id) {
                        Ok(snapshot) if snapshot.is_terminal => {
                            let _ = send_sse_frame(&sender, snapshot.frame).await;
                            break;
                        }
                        Ok(_) => {
                            if send_sse_frame(&sender, ": keepalive\n\n".to_string()).await.is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        }
    });

    let body = StreamBody::new(ReceiverStream::new(receiver)).boxed();
    let mut response = Response::new(body);
    *response.status_mut() = StatusCode::OK;
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn send_sse_frame(
    sender: &mpsc::Sender<Result<Frame<Bytes>, Infallible>>,
    frame: String,
) -> Result<(), mpsc::error::SendError<Result<Frame<Bytes>, Infallible>>> {
    sender.send(Ok(Frame::data(Bytes::from(frame)))).await
}

struct FeishuLoginEventSnapshot {
    frame: String,
    is_terminal: bool,
    version: u64,
    notify: Arc<Notify>,
}

fn feishu_login_event_snapshot(
    runtime: &FeishuLoginRuntime,
    session_id: &str,
) -> Result<FeishuLoginEventSnapshot, FacadeResponse> {
    let mut sessions = runtime.sessions.lock().expect("feishu login session mutex");
    let Some(session) = sessions.get_mut(session_id) else {
        return Err(json_facade_response(
            StatusCode::NOT_FOUND,
            json!({
                "error": "feishu_auth_session_not_found",
                "safe_message": "Feishu login session was not found."
            }),
        ));
    };
    expire_session_if_needed(session);
    let status = session_status_json(session);
    let event = auth_event_name(&status);
    let is_terminal = auth_event_is_terminal(event);
    Ok(FeishuLoginEventSnapshot {
        frame: format!(
            "event: {event}\ndata: {}\n\n",
            auth_event_json(session_id, event, &status)
        ),
        is_terminal,
        version: session.event_version,
        notify: Arc::clone(&session.notify),
    })
}

fn auth_event_name(status: &Value) -> &'static str {
    match status.get("status").and_then(Value::as_str) {
        Some("pending") => "pending",
        Some("authorized") => "authorized",
        Some("denied") => "denied",
        Some("expired") => "expired",
        _ => "keepalive",
    }
}

fn auth_event_is_terminal(event: &str) -> bool {
    matches!(event, "authorized" | "denied" | "expired")
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
    if let Some(error) = params.get("error").filter(|value| !value.trim().is_empty()) {
        let safe_message = if error == "access_denied" {
            "飞书扫码授权已取消。"
        } else {
            "飞书授权失败，请重新扫码。"
        };
        mark_session_denied(runtime, state, safe_message);
        return callback_html(StatusCode::OK, "授权未完成", safe_message);
    }
    let Some(code) = params.get("code").filter(|value| !value.trim().is_empty()) else {
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
        match &session.state {
            FeishuLoginSessionState::Denied { safe_message } => {
                return callback_html(StatusCode::GONE, "登录会话未完成", safe_message);
            }
            FeishuLoginSessionState::Expired => {
                return callback_html(
                    StatusCode::GONE,
                    "登录会话已失效",
                    "这次登录会话已失效，请回到 OAR 重新发起登录。",
                );
            }
            FeishuLoginSessionState::Pending | FeishuLoginSessionState::Authorized { .. } => {}
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
    if let Err(error) =
        persist_feishu_login_grant(runtime.grant_persistence.as_ref(), &login, &oar_session_id)
            .await
    {
        tracing::warn!(?error, "feishu oauth login grant persistence failed");
        mark_session_denied(runtime, state, "OAR 登录态保存失败，请重新扫码。");
        return callback_html(
            StatusCode::SERVICE_UNAVAILABLE,
            "OAR 登录暂不可用",
            "OAR 登录态保存失败，请回到客户端重新发起登录。",
        );
    }
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
        notify_session_changed(session);
    }

    callback_html(StatusCode::OK, "OAR 登录成功", "可以回到 OAR 客户端继续。")
}

async fn persist_feishu_login_grant(
    persistence: Option<&FeishuGrantPersistenceRuntime>,
    login: &FeishuOAuthLogin,
    oar_session_id: &str,
) -> Result<(), FeishuLoginPersistenceError> {
    let Some(persistence) = persistence else {
        return Ok(());
    };

    let plan = build_feishu_login_persistence_plan(
        login,
        oar_session_id,
        &persistence.grant_key_id,
        persistence.grant_key_material,
        SystemTime::now(),
    )?;
    PostgresTenantRepository::new(persistence.pool.clone())
        .upsert(&plan.tenant)
        .await
        .map_err(|_| FeishuLoginPersistenceError::StoreFailed { stage: "tenant" })?;
    PostgresWorkspaceUserRepository::new(persistence.pool.clone())
        .upsert(&plan.user)
        .await
        .map_err(|_| FeishuLoginPersistenceError::StoreFailed {
            stage: "workspace_user",
        })?;
    PostgresLarkIdentityRepository::new(persistence.pool.clone())
        .upsert(&plan.identity)
        .await
        .map_err(|_| FeishuLoginPersistenceError::StoreFailed {
            stage: "lark_identity",
        })?;
    PostgresTokenGrantRepository::new(persistence.pool.clone())
        .upsert_encrypted_grant(&plan.grant)
        .await
        .map_err(|_| FeishuLoginPersistenceError::StoreFailed {
            stage: "token_grant",
        })?;
    PostgresDeviceSessionRepository::new(persistence.pool.clone())
        .upsert_with_identity_hash(&plan.session, &plan.session_identity_hash)
        .await
        .map_err(|_| FeishuLoginPersistenceError::StoreFailed {
            stage: "device_session",
        })?;
    Ok(())
}

fn build_feishu_login_persistence_plan(
    login: &FeishuOAuthLogin,
    oar_session_id: &str,
    grant_key_id: &str,
    grant_key_material: [u8; 32],
    now: SystemTime,
) -> Result<FeishuLoginPersistencePlan, FeishuLoginPersistenceError> {
    let tenant_key = login
        .user
        .tenant_key
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or(FeishuLoginPersistenceError::MissingTenantKey)?;
    let refresh_token = login
        .token
        .refresh_token
        .clone()
        .ok_or(FeishuLoginPersistenceError::MissingRefreshToken)?;

    let tenant_id = TenantId(stable_prefixed_id("feishu_tenant", &[tenant_key]));
    let user_id = WorkspaceUserId(stable_prefixed_id(
        "feishu_user",
        &[tenant_key, &login.user.open_id],
    ));
    let identity_id = LarkIdentityId(stable_prefixed_id(
        "feishu_identity",
        &[tenant_key, &login.user.open_id],
    ));
    let grant_id = stable_prefixed_id("feishu_grant", &[tenant_key, &login.user.open_id]);
    let mut encryptor = AesGcmGrantEncryptor::new(grant_key_id.to_string(), grant_key_material);
    let envelope = encryptor
        .encrypt(FeishuGrantEncryptionInput {
            grant_id: grant_id.clone(),
            tenant_id: tenant_id.0.clone(),
            expected_fingerprint: "initial_login".to_string(),
            access_token: login.token.access_token.clone(),
            refresh_token,
            expires_in_seconds: login.token.expires_in_seconds,
            refresh_token_expires_in_seconds: login.token.refresh_token_expires_in_seconds,
            token_type: login.token.token_type.clone(),
            scope: login.token.scope.clone(),
        })
        .map_err(|_| FeishuLoginPersistenceError::EncryptGrantFailed)?;
    let encrypted_oauth_grant =
        compose_encrypted_grant_blob(envelope.encrypted_primary, envelope.encrypted_renewal);
    let issued_at_ms = system_time_to_ms_lossy(now);
    let grant = EncryptedTokenGrantRecord {
        id: grant_id,
        tenant_id: tenant_id.0.clone(),
        identity_id: identity_id.0.clone(),
        actor_kind: ActorKind::User,
        scope_boundary: ScopeBoundary::User,
        scopes: oauth_scope_list(login.token.scope.as_deref()),
        state: TokenGrantState::Valid,
        issued_at_ms,
        expires_at_ms: envelope.expires_at_ms,
        refreshed_at_ms: Some(envelope.refreshed_at_ms),
        revoked_at_ms: None,
        reauth_required_at_ms: None,
        last_refresh_error: None,
        encrypted_oauth_grant,
        oauth_grant_key_id: envelope.key_id,
        oauth_grant_fingerprint: envelope.new_fingerprint,
        revocation_reason: None,
    };
    let session = DeviceSession::new(
        DeviceSessionId(oar_session_id.to_string()),
        tenant_id.clone(),
        user_id.clone(),
        DeviceEntryPoint::MacOs,
        "review_inbox",
        0,
        now,
    );
    let session_identity_hash = stable_sha256_hex(&[&tenant_id.0, &user_id.0, oar_session_id]);

    Ok(FeishuLoginPersistencePlan {
        tenant: Tenant {
            id: tenant_id.clone(),
            display_name: tenant_key.to_string(),
            status: TenantStatus::Active,
        },
        user: WorkspaceUser {
            id: user_id,
            tenant_id: tenant_id.clone(),
            display_name: login.user.display_name.clone(),
            status: WorkspaceUserStatus::Active,
        },
        identity: LarkIdentity {
            id: identity_id,
            tenant_id,
            actor_kind: ActorKind::User,
            actor_external_id: login.user.open_id.clone(),
            display_name: Some(login.user.display_name.clone()),
        },
        grant,
        session,
        session_identity_hash,
    })
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
        notify_session_changed(session);
    }
}

fn expire_session_if_needed(session: &mut FeishuLoginSession) {
    if matches!(session.state, FeishuLoginSessionState::Pending)
        && SystemTime::now() > session.expires_at
    {
        session.state = FeishuLoginSessionState::Expired;
        notify_session_changed(session);
    }
}

fn notify_session_changed(session: &mut FeishuLoginSession) {
    session.event_version = session.event_version.saturating_add(1);
    session.notify.notify_waiters();
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

fn accepts_event_stream(accept: Option<&str>) -> bool {
    accept
        .map(|value| {
            value
                .split(',')
                .any(|part| part.trim().starts_with("text/event-stream"))
        })
        .unwrap_or(false)
}

fn non_empty_env(env: &impl Fn(&str) -> Option<String>, key: &str) -> Option<String> {
    env(key).and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn oauth_scope_list(scope: Option<&str>) -> Vec<String> {
    scope
        .into_iter()
        .flat_map(str::split_whitespace)
        .filter(|scope| !scope.trim().is_empty())
        .map(ToOwned::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn stable_prefixed_id(prefix: &str, parts: &[&str]) -> String {
    const MAX_FRAGMENT_CHARS: usize = 96;

    let fragment = parts
        .iter()
        .map(|part| sanitize_id_fragment(part))
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    let digest = stable_sha256_hex(parts);
    if fragment.is_empty() {
        return format!("{prefix}_{}", &digest[..16]);
    }
    if fragment.chars().count() <= MAX_FRAGMENT_CHARS {
        return format!("{prefix}_{fragment}");
    }

    let shortened = fragment
        .chars()
        .take(MAX_FRAGMENT_CHARS)
        .collect::<String>();
    format!("{prefix}_{shortened}_{}", &digest[..16])
}

fn sanitize_id_fragment(value: &str) -> String {
    let trimmed = value.trim().trim_matches('_');
    let mut out = String::with_capacity(trimmed.len());
    let mut previous_was_separator = false;
    for character in trimmed.chars() {
        let next = if character.is_ascii_alphanumeric() {
            previous_was_separator = false;
            Some(character)
        } else if character == '-' || character == '_' {
            if previous_was_separator {
                None
            } else {
                previous_was_separator = true;
                Some('_')
            }
        } else if previous_was_separator {
            None
        } else {
            previous_was_separator = true;
            Some('_')
        };
        if let Some(next) = next {
            out.push(next);
        }
    }
    out.trim_matches('_').to_string()
}

fn stable_sha256_hex(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn system_time_to_ms_lossy(time: SystemTime) -> u64 {
    let millis = time
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_millis();
    millis.min(u128::from(u64::MAX)) as u64
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
    use oar_lark_adapter::{FeishuOAuthLoginToken, FeishuOAuthLoginUser, SecretString};

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
        assert!(body["qr_page_url"]
            .as_str()
            .expect("qr url")
            .contains("scope=offline_access"));
        assert!(!body["qr_page_url"]
            .as_str()
            .expect("qr url")
            .contains("auth%3Auser.id%3Aread"));
        assert!(!response.body.contains("super-secret"));
    }

    #[tokio::test]
    async fn async_runtime_requires_grant_key_config_when_database_is_enabled() {
        let error = OarHttpFacadeRuntime::from_env_map_async(&|key| match key {
            "DATABASE_URL" => Some("postgres://oar:oar@127.0.0.1:5432/oar".to_string()),
            "OAR_FEISHU_APP_ID" => Some("cli_test".to_string()),
            "OAR_FEISHU_APP_SECRET" => Some("super-secret".to_string()),
            "OAR_FEISHU_REDIRECT_URI" => {
                Some("https://oar.example.test/auth/feishu/callback".to_string())
            }
            _ => None,
        })
        .await
        .expect_err("database-backed login requires grant encryption key config");

        assert_eq!(error.to_string(), "oar_feishu_grant_config_invalid");
        assert!(!format!("{error:?}").contains("super-secret"));
    }

    #[test]
    fn feishu_login_persistence_plan_builds_stable_redacted_grant() {
        let login = sample_feishu_login(Some("refresh-token-sensitive"));
        let plan = build_feishu_login_persistence_plan(
            &login,
            "oar_session_abc",
            "key-prod-v1",
            [7; 32],
            UNIX_EPOCH + Duration::from_secs(1),
        )
        .expect("plan");

        assert_eq!(plan.tenant.id.0, "feishu_tenant_tenant_1");
        assert_eq!(plan.user.id.0, "feishu_user_tenant_1_ou_123");
        assert_eq!(plan.identity.actor_external_id, "ou_123");
        assert_eq!(plan.grant.identity_id, plan.identity.id.0);
        assert_eq!(plan.grant.scope_boundary, ScopeBoundary::User);
        assert_eq!(
            plan.grant.scopes,
            vec!["auth:user.id:read", "offline_access"]
        );
        assert_eq!(plan.grant.state, TokenGrantState::Valid);
        assert_eq!(plan.grant.issued_at_ms, 1_000);
        assert!(plan.grant.refreshed_at_ms.is_some());
        assert!(plan.grant.expires_at_ms.is_some());
        assert!(plan.grant.encrypted_oauth_grant.len() > "access-token-sensitive".len());
        assert_eq!(plan.session.id.0, "oar_session_abc");
        assert_eq!(plan.session_identity_hash.len(), 64);

        let grant_debug = format!("{:?}", plan.grant);
        assert!(!grant_debug.contains("access-token-sensitive"));
        assert!(!grant_debug.contains("refresh-token-sensitive"));
        assert!(!grant_debug.contains("key-prod-v1"));
        assert!(!grant_debug.contains(&plan.grant.oauth_grant_fingerprint));
        assert!(!contains_bytes(
            &plan.grant.encrypted_oauth_grant,
            b"access-token-sensitive"
        ));
        assert!(!contains_bytes(
            &plan.grant.encrypted_oauth_grant,
            b"refresh-token-sensitive"
        ));
    }

    #[test]
    fn feishu_login_persistence_plan_requires_refresh_token() {
        let login = sample_feishu_login(None);
        let error = build_feishu_login_persistence_plan(
            &login,
            "oar_session_abc",
            "key-prod-v1",
            [7; 32],
            UNIX_EPOCH,
        )
        .expect_err("refresh token required");

        assert_eq!(error, FeishuLoginPersistenceError::MissingRefreshToken);
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

    #[tokio::test]
    async fn hyper_sse_stream_pushes_authorized_event_when_session_changes() {
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

        let create = create_feishu_login_session(runtime.feishu_login.as_deref());
        let created: Value = serde_json::from_str(&create.body).expect("create json");
        let session_id = created["session_id"].as_str().expect("session id");
        let response = feishu_login_session_event_stream_response(
            runtime.feishu_login.clone(),
            session_id.to_string(),
        );

        authorize_test_session(&runtime, session_id);

        let collected = time::timeout(Duration::from_secs(1), response.into_body().collect())
            .await
            .expect("stream should complete")
            .expect("body should collect");
        let body = String::from_utf8(collected.to_bytes().to_vec()).expect("utf8 body");

        assert_eq!(create.status, StatusCode::CREATED);
        assert!(body.contains("event: pending"));
        assert!(body.contains("event: authorized"));
        assert!(body.contains("\"session_id\":\"mock-oar-session\""));
        assert!(!body.contains("super-secret"));
    }

    #[tokio::test]
    async fn callback_without_code_does_not_invalidate_pending_login_session() {
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
        let session_id = created["session_id"].as_str().expect("session id");

        let callback = dispatch_request_with_runtime(
            Arc::clone(&runtime),
            &Method::GET,
            "/auth/feishu/callback",
            Some(&format!("state={session_id}")),
            None,
            None,
        )
        .await;
        let poll = dispatch_request_with_runtime(
            runtime,
            &Method::GET,
            &format!("/auth/feishu/qr-sessions/{session_id}"),
            None,
            None,
            None,
        )
        .await;
        let status: Value = serde_json::from_str(&poll.body).expect("poll json");

        assert_eq!(callback.status, StatusCode::BAD_REQUEST);
        assert_eq!(poll.status, StatusCode::OK);
        assert_eq!(status["status"], "pending");
        assert_eq!(status["safe_message"], Value::Null);
        assert!(!callback.body.contains("super-secret"));
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
    fn bearer_session_id_requires_oar_session_prefix() {
        assert_eq!(
            bearer_session_id(Some("Bearer oar_session_abc")).expect("session"),
            "oar_session_abc"
        );
        assert_eq!(
            bearer_session_id(Some("Bearer other_token")).expect_err("invalid"),
            OarSessionAuthError::InvalidSession
        );
        assert_eq!(
            bearer_session_id(None).expect_err("missing"),
            OarSessionAuthError::MissingBearer
        );
    }

    #[test]
    fn authenticated_context_requires_active_device_session() {
        let active = stored_device_session(SessionState::Active, None, None);
        let context = authenticated_context_from_session(&active).expect("active context");

        assert_eq!(context.session_id, "oar_session_test");
        assert_eq!(context.tenant_id, "tenant_1");
        assert_eq!(context.user_id, "user_1");

        let revoked = stored_device_session(SessionState::Revoked, Some(UNIX_EPOCH), None);
        assert_eq!(
            authenticated_context_from_session(&revoked).expect_err("revoked"),
            OarSessionAuthError::InvalidSession
        );

        let expired = stored_device_session(SessionState::Expired, None, Some(UNIX_EPOCH));
        assert_eq!(
            authenticated_context_from_session(&expired).expect_err("expired"),
            OarSessionAuthError::InvalidSession
        );
    }

    #[test]
    fn snapshot_requires_verified_oar_session_store() {
        let unauthorized = dispatch_request(&Method::GET, "/review-inbox/snapshot", None, None);
        assert_eq!(unauthorized.status, StatusCode::UNAUTHORIZED);

        let response = dispatch_request(
            &Method::GET,
            "/review-inbox/snapshot",
            Some("Bearer oar_session_dev"),
            Some("application/json"),
        );
        let body: Value = serde_json::from_str(&response.body).expect("json");

        assert_eq!(response.status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["error"], "oar_session_verification_unavailable");
    }

    #[test]
    fn decisions_require_verified_oar_session_store() {
        let response = dispatch_request(
            &Method::POST,
            "/review-inbox/decisions",
            Some("Bearer oar_session_dev"),
            Some("application/json"),
        );
        let body: Value = serde_json::from_str(&response.body).expect("json");

        assert_eq!(response.status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["error"], "oar_session_verification_unavailable");
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

    fn sample_feishu_login(refresh_token: Option<&str>) -> FeishuOAuthLogin {
        FeishuOAuthLogin {
            token: FeishuOAuthLoginToken {
                access_token: SecretString::new("access-token-sensitive"),
                refresh_token: refresh_token.map(SecretString::new),
                expires_in_seconds: 7_200,
                refresh_token_expires_in_seconds: Some(30 * 86_400),
                token_type: Some("Bearer".to_string()),
                scope: Some("offline_access auth:user.id:read offline_access".to_string()),
            },
            user: FeishuOAuthLoginUser {
                open_id: "ou_123".to_string(),
                union_id: Some("on_123".to_string()),
                tenant_key: Some("tenant_1".to_string()),
                display_name: "Alice".to_string(),
            },
        }
    }

    fn authorize_test_session(runtime: &OarHttpFacadeRuntime, session_id: &str) {
        let login_runtime = runtime.feishu_login.as_ref().expect("feishu login runtime");
        let mut sessions = login_runtime
            .sessions
            .lock()
            .expect("feishu login session mutex");
        let session = sessions.get_mut(session_id).expect("session");
        session.state = FeishuLoginSessionState::Authorized {
            oar_session_id: "mock-oar-session".to_string(),
            user_id: "ou_123".to_string(),
            display_name: "陈敏".to_string(),
            tenant_name: "tenant_1".to_string(),
        };
        notify_session_changed(session);
    }

    fn stored_device_session(
        state: SessionState,
        revoked_at: Option<SystemTime>,
        expired_at: Option<SystemTime>,
    ) -> StoredDeviceSession {
        StoredDeviceSession {
            id: "oar_session_test".to_string(),
            tenant_id: "tenant_1".to_string(),
            user_id: "user_1".to_string(),
            entry_point: DeviceEntryPoint::MacOs,
            state,
            sync_stream: "review_inbox".to_string(),
            sync_cursor_value: 0,
            sync_cursor_updated_at: UNIX_EPOCH,
            session_identity_hash: "hash".to_string(),
            last_seen_at: UNIX_EPOCH,
            revoked_at,
            expired_at,
        }
    }

    fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }
}

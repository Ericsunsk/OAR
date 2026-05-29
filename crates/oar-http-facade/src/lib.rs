#![forbid(unsafe_code)]

mod agent;
mod agent_routes;
mod config;
mod feishu_auth;
mod response;
mod session_auth;
mod util;

use std::convert::Infallible;
use std::error::Error;
use std::fmt;
use std::sync::Arc;

use hyper::body::Incoming;
use hyper::header::{ACCEPT, AUTHORIZATION};
use hyper::http::{Method, StatusCode};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use oar_lark_adapter::PostgresFeishuAuthRefreshEnvConfig;
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use tracing::{error, info};

use agent::{AgentModelSettingsRuntime, AgentRuntime, AgentRuntimeConfigError};
use feishu_auth::{
    auth_session_events_id, auth_session_status_id, complete_feishu_login_callback,
    create_feishu_login_session, feishu_login_session_event,
    feishu_login_session_event_stream_response, feishu_login_session_status,
    is_auth_session_events_route, is_auth_session_status_route, FeishuGrantPersistenceRuntime,
    FeishuLoginRuntime, FeishuLoginRuntimeConfigError,
};
use response::{
    json_facade_response, not_found, not_implemented, service_unavailable, ResponseBody,
};
pub(crate) use session_auth::{
    authenticate_oar_session, oar_session_auth_error_response,
    protected_route_requires_session_store, AuthenticatedContext,
};
#[cfg(test)]
pub(crate) use session_auth::{
    authenticated_context_from_session, bearer_session_id, OarSessionAuthError,
};
use util::non_empty_env;

pub use config::{OarHttpFacadeConfig, OarHttpFacadeConfigError};
pub use response::FacadeResponse;

#[derive(Clone, Default)]
pub struct OarHttpFacadeRuntime {
    feishu_login: Option<Arc<FeishuLoginRuntime>>,
    agent: Option<Arc<AgentRuntime>>,
    agent_settings: Option<Arc<AgentModelSettingsRuntime>>,
}

impl fmt::Debug for OarHttpFacadeRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OarHttpFacadeRuntime")
            .field("feishu_login", &self.feishu_login.is_some())
            .field("agent", &self.agent.is_some())
            .field("agent_settings", &self.agent_settings.is_some())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OarHttpFacadeRuntimeError {
    PartialFeishuAuthConfig,
    InvalidFeishuOpenApiConfig,
    InvalidFeishuLoginConfig,
    InvalidFeishuGrantConfig,
    PartialAgentConfig,
    InvalidAgentConfig,
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
            Self::PartialAgentConfig => write!(f, "oar_agent_config_partial"),
            Self::InvalidAgentConfig => write!(f, "oar_agent_config_invalid"),
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
            Some(FeishuGrantPersistenceRuntime::new(
                pool,
                grant_config.grant_key_id,
                grant_config.grant_key_material,
            )),
        )
    }

    fn from_env_map_with_persistence(
        env: &impl Fn(&str) -> Option<String>,
        grant_persistence: Option<FeishuGrantPersistenceRuntime>,
    ) -> Result<Self, OarHttpFacadeRuntimeError> {
        let agent = AgentRuntime::from_env_map(env)
            .map_err(agent_runtime_config_error)?
            .map(Arc::new);
        let agent_settings = match grant_persistence.as_ref() {
            Some(persistence) => Some(Arc::new(
                AgentModelSettingsRuntime::new(
                    persistence.pool(),
                    persistence.grant_key_id().to_string(),
                    persistence.grant_key_material(),
                )
                .map_err(agent_runtime_config_error)?,
            )),
            None => None,
        };
        let feishu_login = FeishuLoginRuntime::from_env_map(env, grant_persistence)
            .map_err(feishu_runtime_config_error)?
            .map(Arc::new);
        Ok(Self {
            feishu_login,
            agent,
            agent_settings,
        })
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
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let accept = request
        .headers()
        .get(ACCEPT)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    if agent_routes::is_body_route(&method, &path) {
        return Ok(agent_routes::body_route_response(
            runtime,
            &method,
            &path,
            authorization.as_deref(),
            accept.as_deref(),
            request.into_body(),
        )
        .await);
    }

    if is_auth_session_events_route(&method, &path) {
        if !accepts_event_stream(accept.as_deref()) {
            return Ok(event_stream_required(
                "Auth session events require Accept: text/event-stream.",
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
        authorization.as_deref(),
        accept.as_deref(),
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
                return event_stream_required(
                    "Auth session events require Accept: text/event-stream.",
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
        _ if agent_routes::is_facade_route(method, path) => {
            return agent_routes::facade_route_response(&runtime, method, path, authorization)
                .await;
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
        _ if agent_routes::is_route(method, path) => protected_route_requires_session_store(
            authorization,
            "Agent routes require verified OAR session storage.",
        ),
        _ if is_auth_session_status_route(method, path) => service_unavailable(
            "feishu_auth_not_configured",
            "Feishu QR login is not configured in this backend facade.",
        ),
        _ if is_auth_session_events_route(method, path) => {
            if !accepts_event_stream(accept) {
                return event_stream_required(
                    "Auth session events require Accept: text/event-stream.",
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

fn accepts_event_stream(accept: Option<&str>) -> bool {
    accept
        .map(|value| {
            value
                .split(',')
                .any(|part| part.trim().starts_with("text/event-stream"))
        })
        .unwrap_or(false)
}

fn event_stream_required(safe_message: &'static str) -> FacadeResponse {
    json_facade_response(
        StatusCode::NOT_ACCEPTABLE,
        json!({
            "error": "event_stream_required",
            "safe_message": safe_message
        }),
    )
}

fn agent_runtime_config_error(error: AgentRuntimeConfigError) -> OarHttpFacadeRuntimeError {
    match error {
        AgentRuntimeConfigError::PartialOpenAICompatibleConfig => {
            OarHttpFacadeRuntimeError::PartialAgentConfig
        }
        AgentRuntimeConfigError::PartialAnthropicConfig => {
            OarHttpFacadeRuntimeError::PartialAgentConfig
        }
        AgentRuntimeConfigError::InvalidOpenAICompatibleBaseURL
        | AgentRuntimeConfigError::InvalidAnthropicBaseURL
        | AgentRuntimeConfigError::InvalidAgentProvider
        | AgentRuntimeConfigError::AmbiguousAgentProviderConfig
        | AgentRuntimeConfigError::HttpClientBuildFailed => {
            OarHttpFacadeRuntimeError::InvalidAgentConfig
        }
    }
}

fn feishu_runtime_config_error(error: FeishuLoginRuntimeConfigError) -> OarHttpFacadeRuntimeError {
    match error {
        FeishuLoginRuntimeConfigError::PartialAuthConfig => {
            OarHttpFacadeRuntimeError::PartialFeishuAuthConfig
        }
        FeishuLoginRuntimeConfigError::InvalidOpenApiConfig => {
            OarHttpFacadeRuntimeError::InvalidFeishuOpenApiConfig
        }
        FeishuLoginRuntimeConfigError::InvalidLoginConfig => {
            OarHttpFacadeRuntimeError::InvalidFeishuLoginConfig
        }
        FeishuLoginRuntimeConfigError::HttpClientBuildFailed => {
            OarHttpFacadeRuntimeError::HttpClientBuildFailed
        }
    }
}

#[cfg(test)]
mod tests;

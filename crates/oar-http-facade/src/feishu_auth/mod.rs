use std::collections::HashMap;
use std::convert::Infallible;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use bytes::Bytes;
use http_body_util::{BodyExt, StreamBody};
use hyper::body::Frame;
use hyper::header::{CACHE_CONTROL, CONTENT_TYPE};
use hyper::http::{HeaderValue, Method, StatusCode};
use hyper::Response;
use oar_core::action::capability::default_agent_feishu_oauth_scope_strings;
use oar_lark_adapter::{
    AsyncFeishuOAuthLogin, FeishuOAuthLoginClient, FeishuOAuthLoginConfig, FeishuOpenApiConfig,
    ReqwestAsyncHttpClient,
};
use serde_json::json;
use sqlx::PgPool;
use tokio::sync::{mpsc, Notify};
use tokio::time;
use tokio_stream::wrappers::ReceiverStream;

mod persistence;
mod session;
mod util;

use persistence::persist_feishu_login_grant;
#[cfg(test)]
pub(crate) use persistence::{build_feishu_login_persistence_plan, FeishuLoginPersistenceError};
use session::{
    auth_event_is_terminal, auth_event_json, auth_event_name, expire_session_if_needed,
    mark_session_denied, notify_session_changed, qr_session_json, session_status_json,
    FeishuLoginSession, FeishuLoginSessionState,
};
pub(crate) use util::iso8601_utc;
use util::{parse_query, sanitize_session_suffix, secure_random_hex};

use crate::response::{
    callback_html, json_facade_response, service_unavailable, sse_facade_response, FacadeResponse,
    ResponseBody,
};
use crate::util::non_empty_env;

const FEISHU_LOGIN_SSE_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FeishuLoginRuntimeConfigError {
    PartialAuthConfig,
    InvalidOpenApiConfig,
    InvalidLoginConfig,
    HttpClientBuildFailed,
}

impl FeishuLoginRuntime {
    pub(crate) fn from_env_map(
        env: &impl Fn(&str) -> Option<String>,
        grant_persistence: Option<FeishuGrantPersistenceRuntime>,
    ) -> Result<Option<Self>, FeishuLoginRuntimeConfigError> {
        let app_id = non_empty_env(env, "OAR_FEISHU_APP_ID");
        let app_secret = non_empty_env(env, "OAR_FEISHU_APP_SECRET");
        let redirect_uri = non_empty_env(env, "OAR_FEISHU_REDIRECT_URI");
        let has_any_auth_config =
            app_id.is_some() || app_secret.is_some() || redirect_uri.is_some();
        if !has_any_auth_config {
            return Ok(None);
        }

        let (Some(app_id), Some(app_secret), Some(redirect_uri)) =
            (app_id, app_secret, redirect_uri)
        else {
            return Err(FeishuLoginRuntimeConfigError::PartialAuthConfig);
        };

        let open_api = FeishuOpenApiConfig::from_env_map(env)
            .map_err(|_| FeishuLoginRuntimeConfigError::InvalidOpenApiConfig)?;
        let authorize_base_url = non_empty_env(env, "OAR_FEISHU_AUTHORIZE_BASE_URL")
            .unwrap_or_else(|| "https://open.feishu.cn".to_string());
        let scope = non_empty_env(env, "OAR_FEISHU_AUTH_SCOPE")
            .or_else(|| Some(default_feishu_auth_scope()));
        let config = FeishuOAuthLoginConfig::new(
            open_api.clone(),
            authorize_base_url,
            app_id,
            app_secret,
            redirect_uri,
            scope,
        )
        .map_err(|_| FeishuLoginRuntimeConfigError::InvalidLoginConfig)?;
        let http_client = ReqwestAsyncHttpClient::with_config(&open_api)
            .map_err(|_| FeishuLoginRuntimeConfigError::HttpClientBuildFailed)?;
        Ok(Some(Self {
            config,
            http_client,
            grant_persistence,
            sessions: Mutex::new(HashMap::new()),
        }))
    }

    pub(crate) fn grant_persistence(&self) -> Option<&FeishuGrantPersistenceRuntime> {
        self.grant_persistence.as_ref()
    }

    pub(crate) fn open_api_config(&self) -> FeishuOpenApiConfig {
        self.config.open_api.clone()
    }

    pub(crate) fn client_id(&self) -> &str {
        &self.config.client_id
    }

    pub(crate) fn client_secret(&self) -> oar_lark_adapter::SecretString {
        self.config.client_secret.clone()
    }
}

fn default_feishu_auth_scope() -> String {
    default_agent_feishu_oauth_scope_strings().join(" ")
}

impl FeishuGrantPersistenceRuntime {
    pub(crate) fn new(pool: PgPool, grant_key_id: String, grant_key_material: [u8; 32]) -> Self {
        Self {
            pool,
            grant_key_id,
            grant_key_material,
        }
    }

    pub(crate) fn pool(&self) -> PgPool {
        self.pool.clone()
    }

    pub(crate) fn grant_key_id(&self) -> &str {
        &self.grant_key_id
    }

    pub(crate) fn grant_key_material(&self) -> [u8; 32] {
        self.grant_key_material
    }
}

pub(crate) struct FeishuLoginRuntime {
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
pub(crate) struct FeishuGrantPersistenceRuntime {
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

pub(crate) fn create_feishu_login_session(runtime: Option<&FeishuLoginRuntime>) -> FacadeResponse {
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

pub(crate) fn feishu_login_session_status(
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

pub(crate) fn feishu_login_session_event(
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

pub(crate) fn feishu_login_session_event_stream_response(
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

pub(crate) async fn complete_feishu_login_callback(
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

pub(crate) fn auth_session_status_id(path: &str) -> Option<&str> {
    path.strip_prefix("/auth/feishu/qr-sessions/")
        .filter(|session_id| !session_id.is_empty() && !session_id.ends_with("/events"))
}

pub(crate) fn auth_session_events_id(path: &str) -> Option<&str> {
    path.strip_prefix("/auth/feishu/qr-sessions/")
        .and_then(|session_id| session_id.strip_suffix("/events"))
        .filter(|session_id| !session_id.is_empty())
}

pub(crate) fn is_auth_session_status_route(method: &Method, path: &str) -> bool {
    *method == Method::GET && auth_session_status_id(path).is_some()
}

pub(crate) fn is_auth_session_events_route(method: &Method, path: &str) -> bool {
    *method == Method::GET && auth_session_events_id(path).is_some()
}

#[cfg(test)]
pub(crate) fn authorize_test_session(runtime: &FeishuLoginRuntime, session_id: &str) {
    let mut sessions = runtime.sessions.lock().expect("feishu login session mutex");
    let session = sessions.get_mut(session_id).expect("session");
    session.state = FeishuLoginSessionState::Authorized {
        oar_session_id: "mock-oar-session".to_string(),
        user_id: "ou_123".to_string(),
        display_name: "陈敏".to_string(),
        tenant_name: "tenant_1".to_string(),
    };
    notify_session_changed(session);
}

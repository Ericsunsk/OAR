use std::sync::Arc;
use std::time::{Duration, SystemTime};

use hyper::http::StatusCode;
use oar_lark_adapter::{AsyncFeishuOAuthLogin, FeishuOAuthLoginClient};
use serde_json::json;
use tokio::sync::Notify;

use super::persistence::persist_feishu_login_grant;
use super::session::{
    expire_session_if_needed, mark_session_denied, notify_session_changed, qr_session_json,
    session_status_json, FeishuLoginSession, FeishuLoginSessionState,
};
use super::util::{parse_query, sanitize_session_suffix, secure_random_hex};
use super::FeishuLoginRuntime;
use crate::persistence::FacadePersistenceRuntime;
use crate::response::{callback_html, json_facade_response, service_unavailable, FacadeResponse};

fn not_configured_response() -> FacadeResponse {
    service_unavailable(
        "feishu_auth_not_configured",
        "Feishu QR login is not configured in this backend facade.",
    )
}

pub(crate) fn create_feishu_login_session(runtime: Option<&FeishuLoginRuntime>) -> FacadeResponse {
    let Some(runtime) = runtime else {
        return not_configured_response();
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
        return not_configured_response();
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

pub(crate) async fn complete_feishu_login_callback(
    runtime: Option<&FeishuLoginRuntime>,
    persistence: Option<&FacadePersistenceRuntime>,
    query: Option<&str>,
) -> FacadeResponse {
    let Some(runtime) = runtime else {
        return not_configured_response();
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
    if let Err(error) = persist_feishu_login_grant(persistence, &login, &oar_session_id).await {
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

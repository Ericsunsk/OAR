use std::sync::Arc;
use std::time::SystemTime;

use serde_json::{json, Value};
use tokio::sync::Notify;

use super::{iso8601_utc, FeishuLoginRuntime};

#[derive(Debug, Clone)]
pub(super) struct FeishuLoginSession {
    pub(super) id: String,
    pub(super) qr_page_url: String,
    pub(super) expires_at: SystemTime,
    pub(super) state: FeishuLoginSessionState,
    pub(super) event_version: u64,
    pub(super) notify: Arc<Notify>,
}

#[derive(Debug, Clone)]
pub(super) enum FeishuLoginSessionState {
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

pub(super) fn mark_session_denied(
    runtime: &FeishuLoginRuntime,
    session_id: &str,
    safe_message: &str,
) {
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

pub(super) fn expire_session_if_needed(session: &mut FeishuLoginSession) {
    if matches!(session.state, FeishuLoginSessionState::Pending)
        && SystemTime::now() > session.expires_at
    {
        session.state = FeishuLoginSessionState::Expired;
        notify_session_changed(session);
    }
}

pub(super) fn notify_session_changed(session: &mut FeishuLoginSession) {
    session.event_version = session.event_version.saturating_add(1);
    session.notify.notify_waiters();
}

pub(super) fn qr_session_json(session: &FeishuLoginSession) -> Value {
    json!({
        "session_id": session.id,
        "qr_page_url": session.qr_page_url,
        "expires_at": iso8601_utc(session.expires_at)
    })
}

pub(super) fn session_status_json(session: &FeishuLoginSession) -> Value {
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

pub(super) fn auth_event_json(session_id: &str, event: &str, status: &Value) -> Value {
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

pub(super) fn auth_event_name(status: &Value) -> &'static str {
    match status.get("status").and_then(Value::as_str) {
        Some("pending") => "pending",
        Some("authorized") => "authorized",
        Some("denied") => "denied",
        Some("expired") => "expired",
        _ => "keepalive",
    }
}

pub(super) fn auth_event_is_terminal(event: &str) -> bool {
    matches!(event, "authorized" | "denied" | "expired")
}

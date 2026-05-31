use std::sync::Arc;

use hyper::http::{Method, StatusCode};
use serde_json::json;

use crate::agent_routes;
use crate::feishu_auth::{
    auth_session_events_id, auth_session_status_id, complete_feishu_login_callback,
    create_feishu_login_session, feishu_login_session_event, feishu_login_session_status,
    is_auth_session_events_route, is_auth_session_status_route,
};
use crate::health::healthz_response;
use crate::response::{json_facade_response, not_found, service_unavailable, FacadeResponse};
use crate::review_inbox_routes;
use crate::runtime::OarHttpFacadeRuntime;
use crate::session_auth::logout_oar_session;
use crate::{
    authenticate_oar_session, oar_session_auth_error_response,
    protected_route_requires_session_store,
};

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
            return complete_feishu_login_callback(
                runtime.feishu_login.as_deref(),
                runtime.persistence(),
                query,
            )
            .await;
        }
        (&Method::GET, "/healthz") => {
            return healthz_response(Some(runtime.as_ref()));
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
        (&Method::POST, "/auth/logout") => {
            return logout_oar_session(&runtime, authorization).await;
        }
        (&Method::GET, "/review-inbox/snapshot") => {
            let auth_context = match authenticate_oar_session(&runtime, authorization).await {
                Ok(context) => context,
                Err(error) => return oar_session_auth_error_response(error),
            };
            return review_inbox_routes::snapshot_for_context(&runtime, &auth_context).await;
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
        (&Method::GET, "/healthz") => healthz_response(None),
        (&Method::POST, "/auth/feishu/qr-sessions") => service_unavailable(
            "feishu_auth_not_configured",
            "Feishu QR login is not configured in this backend facade.",
        ),
        (&Method::POST, "/auth/logout") => protected_route_requires_session_store(
            authorization,
            "Logout requires verified OAR session storage.",
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

pub(crate) fn accepts_event_stream(accept: Option<&str>) -> bool {
    accept
        .map(|value| {
            value
                .split(',')
                .any(|part| part.trim().starts_with("text/event-stream"))
        })
        .unwrap_or(false)
}

pub(crate) fn event_stream_required(safe_message: &'static str) -> FacadeResponse {
    json_facade_response(
        StatusCode::NOT_ACCEPTABLE,
        json!({
            "error": "event_stream_required",
            "safe_message": safe_message
        }),
    )
}

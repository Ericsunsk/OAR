use hyper::http::Method;

const QR_SESSION_ROUTE_PREFIX: &str = "/auth/feishu/qr-sessions/";

pub(crate) fn auth_session_status_id(path: &str) -> Option<&str> {
    path.strip_prefix(QR_SESSION_ROUTE_PREFIX)
        .filter(|session_id| !session_id.is_empty() && !session_id.ends_with("/events"))
}

pub(crate) fn auth_session_events_id(path: &str) -> Option<&str> {
    path.strip_prefix(QR_SESSION_ROUTE_PREFIX)
        .and_then(|session_id| session_id.strip_suffix("/events"))
        .filter(|session_id| !session_id.is_empty())
}

pub(crate) fn is_auth_session_status_route(method: &Method, path: &str) -> bool {
    *method == Method::GET && auth_session_status_id(path).is_some()
}

pub(crate) fn is_auth_session_events_route(method: &Method, path: &str) -> bool {
    *method == Method::GET && auth_session_events_id(path).is_some()
}

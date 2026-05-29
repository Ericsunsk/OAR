use std::sync::Arc;

use hyper::body::Incoming;
use hyper::http::Method;
use hyper::Response;

use crate::response::{not_found, FacadeResponse, ResponseBody};
use crate::{accepts_event_stream, event_stream_required, OarHttpFacadeRuntime};

mod settings;
mod stream;

pub(crate) fn is_route(method: &Method, path: &str) -> bool {
    is_body_route(method, path) || is_facade_route(method, path)
}

pub(crate) fn is_body_route(method: &Method, path: &str) -> bool {
    stream::is_route(method, path) || settings::is_body_route(method, path)
}

pub(crate) fn is_facade_route(method: &Method, path: &str) -> bool {
    settings::is_facade_route(method, path)
}

pub(crate) async fn body_route_response(
    runtime: Arc<OarHttpFacadeRuntime>,
    method: &Method,
    path: &str,
    authorization: Option<&str>,
    accept: Option<&str>,
    body: Incoming,
) -> Response<ResponseBody> {
    if stream::is_route(method, path) {
        if !accepts_event_stream(accept) {
            return event_stream_required("Agent stream requires Accept: text/event-stream.")
                .into_hyper_response();
        }
        return stream::response(runtime, authorization, body).await;
    }

    if settings::is_body_route(method, path) {
        return settings::body_response(runtime, method, authorization, body)
            .await
            .into_hyper_response();
    }

    not_found().into_hyper_response()
}

pub(crate) async fn facade_route_response(
    runtime: &OarHttpFacadeRuntime,
    method: &Method,
    path: &str,
    authorization: Option<&str>,
) -> FacadeResponse {
    if settings::is_facade_route(method, path) {
        settings::facade_response(runtime, method, path, authorization).await
    } else {
        not_found()
    }
}

#[cfg(test)]
mod tests {
    use hyper::http::Method;

    use super::{is_body_route, is_facade_route, is_route};

    #[test]
    fn route_predicates_cover_only_agent_routes() {
        for (method, path) in [
            (&Method::POST, "/agent/stream"),
            (&Method::POST, "/agent/model-catalog/preview"),
            (&Method::PUT, "/agent/settings"),
        ] {
            assert!(is_body_route(method, path), "{method} {path}");
            assert!(is_route(method, path), "{method} {path}");
        }

        for (method, path) in [
            (&Method::GET, "/agent/settings"),
            (&Method::DELETE, "/agent/settings"),
        ] {
            assert!(is_facade_route(method, path), "{method} {path}");
            assert!(is_route(method, path), "{method} {path}");
        }

        for (method, path) in [
            (&Method::GET, "/agent/stream"),
            (&Method::POST, "/agent/settings"),
            (&Method::GET, "/agent/model-catalog/preview"),
            (&Method::GET, "/healthz"),
        ] {
            assert!(!is_route(method, path), "{method} {path}");
        }
    }
}

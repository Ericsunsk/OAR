use std::convert::Infallible;

use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::header::{CACHE_CONTROL, CONTENT_TYPE};
use hyper::http::{HeaderValue, StatusCode};
use hyper::Response;
use serde_json::{json, Value};

pub(crate) type ResponseBody = BoxBody<Bytes, Infallible>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FacadeResponse {
    pub status: StatusCode,
    pub content_type: &'static str,
    pub body: String,
}

impl FacadeResponse {
    pub(crate) fn into_hyper_response(self) -> Response<ResponseBody> {
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

pub(crate) fn json_facade_response(status: StatusCode, body: Value) -> FacadeResponse {
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

pub(crate) fn sse_facade_response(body: String) -> FacadeResponse {
    FacadeResponse {
        status: StatusCode::OK,
        content_type: "text/event-stream",
        body,
    }
}

pub(crate) fn service_unavailable(
    error: &'static str,
    safe_message: &'static str,
) -> FacadeResponse {
    json_facade_response(
        StatusCode::SERVICE_UNAVAILABLE,
        json!({
            "error": error,
            "safe_message": safe_message
        }),
    )
}

pub(crate) fn callback_html(status: StatusCode, title: &str, message: &str) -> FacadeResponse {
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

pub(crate) fn unauthorized() -> FacadeResponse {
    json_facade_response(
        StatusCode::UNAUTHORIZED,
        json!({
            "error": "missing_oar_session",
            "safe_message": "A valid OAR session bearer token is required."
        }),
    )
}

pub(crate) fn invalid_oar_session() -> FacadeResponse {
    json_facade_response(
        StatusCode::UNAUTHORIZED,
        json!({
            "error": "invalid_oar_session",
            "safe_message": "The OAR session is invalid or expired."
        }),
    )
}

pub(crate) fn not_found() -> FacadeResponse {
    json_facade_response(
        StatusCode::NOT_FOUND,
        json!({
            "error": "not_found",
            "safe_message": "No OAR backend route matched this request."
        }),
    )
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

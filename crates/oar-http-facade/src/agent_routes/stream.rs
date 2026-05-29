use std::sync::Arc;

use http_body_util::{BodyExt, StreamBody};
use hyper::body::Incoming;
use hyper::header::{CACHE_CONTROL, CONTENT_TYPE};
use hyper::http::{HeaderValue, Method, StatusCode};
use hyper::Response;
use serde_json::json;
use tracing::warn;

use crate::agent::{
    decode_agent_stream_request, inject_live_feishu_context, AgentRequestError, AgentRuntime,
    AgentStreamError,
};
use crate::response::{json_facade_response, service_unavailable, FacadeResponse, ResponseBody};
use crate::{
    authenticate_oar_session, oar_session_auth_error_response, AuthenticatedContext,
    OarHttpFacadeRuntime,
};

pub(super) fn is_route(method: &Method, path: &str) -> bool {
    *method == Method::POST && path == "/agent/stream"
}

pub(super) async fn response(
    runtime: Arc<OarHttpFacadeRuntime>,
    authorization: Option<&str>,
    body: Incoming,
) -> Response<ResponseBody> {
    let auth_context = match authenticate_oar_session(&runtime, authorization).await {
        Ok(context) => context,
        Err(error) => return oar_session_auth_error_response(error).into_hyper_response(),
    };
    let body = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(_) => {
            return json_facade_response(
                StatusCode::BAD_REQUEST,
                json!({
                    "error": "agent_request_body_unreadable",
                    "safe_message": "Agent request body could not be read."
                }),
            )
            .into_hyper_response();
        }
    };
    let mut request = match decode_agent_stream_request(&body) {
        Ok(request) => request,
        Err(error) => return agent_request_error_response(error).into_hyper_response(),
    };
    inject_live_feishu_context(&runtime, &auth_context, &mut request).await;

    let user_agent_runtime = user_agent_runtime(&runtime, &auth_context).await;
    let stream = match (&user_agent_runtime, runtime.agent.as_deref()) {
        (Some(agent_runtime), _) => agent_runtime.open_stream(request).await,
        (None, Some(agent_runtime)) => agent_runtime.open_stream(request).await,
        (None, None) => {
            return service_unavailable(
                "agent_model_not_configured",
                "Agent model provider is not configured in this backend facade.",
            )
            .into_hyper_response();
        }
    };
    let stream = match stream {
        Ok(stream) => stream,
        Err(error) => return agent_stream_error_response(error).into_hyper_response(),
    };
    let body = StreamBody::new(stream).boxed();
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

async fn user_agent_runtime(
    runtime: &OarHttpFacadeRuntime,
    auth_context: &AuthenticatedContext,
) -> Option<AgentRuntime> {
    let settings = runtime.agent_settings.as_deref()?;
    let config = match settings
        .provider_config_for_user(&auth_context.tenant_id, &auth_context.user_id)
        .await
    {
        Ok(config) => config?,
        Err(error) => {
            warn!(
                ?error,
                "user agent model settings unavailable; falling back to default agent runtime"
            );
            return None;
        }
    };
    match AgentRuntime::from_provider_config(config) {
        Ok(runtime) => Some(runtime),
        Err(error) => {
            warn!(
                ?error,
                "user agent model settings invalid; falling back to default agent runtime"
            );
            None
        }
    }
}

fn agent_request_error_response(error: AgentRequestError) -> FacadeResponse {
    match error {
        AgentRequestError::InvalidJson => json_facade_response(
            StatusCode::BAD_REQUEST,
            json!({
                "error": "agent_request_invalid_json",
                "safe_message": "Agent request must be valid JSON."
            }),
        ),
    }
}

fn agent_stream_error_response(error: AgentStreamError) -> FacadeResponse {
    match error {
        AgentStreamError::UpstreamUnauthorized => json_facade_response(
            StatusCode::BAD_GATEWAY,
            json!({
                "error": "agent_upstream_unauthorized",
                "safe_message": "Agent model provider authentication failed."
            }),
        ),
        AgentStreamError::UpstreamUnavailable => service_unavailable(
            "agent_upstream_unavailable",
            "Agent model provider is temporarily unavailable.",
        ),
    }
}

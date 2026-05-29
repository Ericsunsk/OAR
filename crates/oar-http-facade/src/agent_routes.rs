use std::sync::Arc;

use http_body_util::{BodyExt, StreamBody};
use hyper::body::Incoming;
use hyper::header::{CACHE_CONTROL, CONTENT_TYPE};
use hyper::http::{HeaderValue, Method, StatusCode};
use hyper::Response;
use serde_json::{json, Value};
use tracing::warn;

use crate::agent::{
    decode_agent_model_catalog_request, decode_agent_settings_update_request,
    decode_agent_stream_request, AgentModelCatalogRequest, AgentModelSettingsError,
    AgentRequestError, AgentRuntime, AgentSettingsUpdateRequest, AgentStreamError,
};
use crate::response::{
    json_facade_response, not_found, service_unavailable, FacadeResponse, ResponseBody,
};
use crate::{
    accepts_event_stream, authenticate_oar_session, event_stream_required,
    oar_session_auth_error_response, AuthenticatedContext, OarHttpFacadeRuntime,
};

pub(crate) fn is_route(method: &Method, path: &str) -> bool {
    is_body_route(method, path) || is_facade_route(method, path)
}

pub(crate) fn is_body_route(method: &Method, path: &str) -> bool {
    is_stream_route(method, path)
        || is_model_catalog_preview_route(method, path)
        || is_settings_update_route(method, path)
}

pub(crate) fn is_facade_route(method: &Method, path: &str) -> bool {
    matches!(
        (method, path),
        (&Method::GET, "/agent/settings") | (&Method::DELETE, "/agent/settings")
    )
}

pub(crate) async fn body_route_response(
    runtime: Arc<OarHttpFacadeRuntime>,
    method: &Method,
    path: &str,
    authorization: Option<&str>,
    accept: Option<&str>,
    body: Incoming,
) -> Response<ResponseBody> {
    if is_stream_route(method, path) {
        if !accepts_event_stream(accept) {
            return event_stream_required("Agent stream requires Accept: text/event-stream.")
                .into_hyper_response();
        }
        return stream_response(runtime, authorization, body).await;
    }

    if is_model_catalog_preview_route(method, path) || is_settings_update_route(method, path) {
        return settings_body_response(runtime, method, authorization, body)
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
    match (method, path) {
        (&Method::GET, "/agent/settings") => {
            let auth_context = match authenticate_oar_session(runtime, authorization).await {
                Ok(context) => context,
                Err(error) => return oar_session_auth_error_response(error),
            };
            settings_snapshot_response(runtime, &auth_context).await
        }
        (&Method::DELETE, "/agent/settings") => {
            let auth_context = match authenticate_oar_session(runtime, authorization).await {
                Ok(context) => context,
                Err(error) => return oar_session_auth_error_response(error),
            };
            delete_settings_response(runtime, &auth_context).await
        }
        _ => not_found(),
    }
}

async fn stream_response(
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
    let request = match decode_agent_stream_request(&body) {
        Ok(request) => request,
        Err(error) => return agent_request_error_response(error).into_hyper_response(),
    };

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

async fn settings_body_response(
    runtime: Arc<OarHttpFacadeRuntime>,
    method: &Method,
    authorization: Option<&str>,
    body: Incoming,
) -> FacadeResponse {
    let auth_context = match authenticate_oar_session(&runtime, authorization).await {
        Ok(context) => context,
        Err(error) => return oar_session_auth_error_response(error),
    };
    let body = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(_) => {
            return json_facade_response(
                StatusCode::BAD_REQUEST,
                json!({
                    "error": "agent_settings_body_unreadable",
                    "safe_message": "Agent settings request body could not be read."
                }),
            );
        }
    };

    match method {
        &Method::POST => {
            let request = match decode_agent_model_catalog_request(&body) {
                Ok(request) => request,
                Err(error) => return agent_model_settings_error_response(error),
            };
            model_catalog_preview_response(&runtime, &auth_context, request).await
        }
        &Method::PUT => {
            let request = match decode_agent_settings_update_request(&body) {
                Ok(request) => request,
                Err(error) => return agent_model_settings_error_response(error),
            };
            save_settings_response(&runtime, &auth_context, request).await
        }
        _ => not_found(),
    }
}

async fn settings_snapshot_response(
    runtime: &OarHttpFacadeRuntime,
    auth_context: &AuthenticatedContext,
) -> FacadeResponse {
    let Some(settings) = runtime.agent_settings.as_deref() else {
        return settings_without_store_response(runtime.agent.as_deref());
    };
    match settings
        .snapshot(
            &auth_context.tenant_id,
            &auth_context.user_id,
            runtime.agent.as_deref(),
        )
        .await
    {
        Ok(snapshot) => json_facade_response(StatusCode::OK, json!(snapshot)),
        Err(error) => agent_model_settings_error_response(error),
    }
}

async fn model_catalog_preview_response(
    runtime: &OarHttpFacadeRuntime,
    auth_context: &AuthenticatedContext,
    request: AgentModelCatalogRequest,
) -> FacadeResponse {
    let Some(settings) = runtime.agent_settings.as_deref() else {
        return service_unavailable(
            "agent_settings_store_unavailable",
            "Agent settings storage is not configured.",
        );
    };
    match settings
        .detect_catalog(&auth_context.tenant_id, &auth_context.user_id, request)
        .await
    {
        Ok(catalog) => json_facade_response(StatusCode::OK, json!(catalog)),
        Err(error) => agent_model_settings_error_response(error),
    }
}

async fn save_settings_response(
    runtime: &OarHttpFacadeRuntime,
    auth_context: &AuthenticatedContext,
    request: AgentSettingsUpdateRequest,
) -> FacadeResponse {
    let Some(settings) = runtime.agent_settings.as_deref() else {
        return service_unavailable(
            "agent_settings_store_unavailable",
            "Agent settings storage is not configured.",
        );
    };
    match settings
        .save_settings(
            &auth_context.tenant_id,
            &auth_context.user_id,
            request,
            runtime.agent.as_deref(),
        )
        .await
    {
        Ok(snapshot) => json_facade_response(StatusCode::OK, json!(snapshot)),
        Err(error) => agent_model_settings_error_response(error),
    }
}

async fn delete_settings_response(
    runtime: &OarHttpFacadeRuntime,
    auth_context: &AuthenticatedContext,
) -> FacadeResponse {
    let Some(settings) = runtime.agent_settings.as_deref() else {
        return settings_without_store_response(runtime.agent.as_deref());
    };
    match settings
        .delete_settings(
            &auth_context.tenant_id,
            &auth_context.user_id,
            runtime.agent.as_deref(),
        )
        .await
    {
        Ok(snapshot) => json_facade_response(StatusCode::OK, json!(snapshot)),
        Err(error) => agent_model_settings_error_response(error),
    }
}

fn settings_without_store_response(default_runtime: Option<&AgentRuntime>) -> FacadeResponse {
    let snapshot = default_runtime
        .map(|runtime| {
            let summary = runtime.config_summary();
            json!({
                "source": "env",
                "detected_protocol": summary.protocol,
                "base_url": summary.base_url,
                "selected_model": summary.model,
                "api_key_status": "saved",
                "can_configure": false
            })
        })
        .unwrap_or_else(|| {
            json!({
                "source": "none",
                "detected_protocol": Value::Null,
                "base_url": Value::Null,
                "selected_model": Value::Null,
                "api_key_status": "missing",
                "can_configure": false
            })
        });
    json_facade_response(StatusCode::OK, snapshot)
}

async fn user_agent_runtime(
    runtime: &OarHttpFacadeRuntime,
    auth_context: &AuthenticatedContext,
) -> Option<AgentRuntime> {
    let Some(settings) = runtime.agent_settings.as_deref() else {
        return None;
    };
    let config = match settings
        .provider_config_for_user(&auth_context.tenant_id, &auth_context.user_id)
        .await
    {
        Ok(None) => return None,
        Ok(Some(config)) => config,
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

fn agent_model_settings_error_response(error: AgentModelSettingsError) -> FacadeResponse {
    match error {
        AgentModelSettingsError::InvalidJson
        | AgentModelSettingsError::MissingBaseURL
        | AgentModelSettingsError::MissingApiKey
        | AgentModelSettingsError::MissingModel
        | AgentModelSettingsError::InvalidBaseURL => json_facade_response(
            StatusCode::BAD_REQUEST,
            json!({
                "error": error.to_string(),
                "safe_message": "Agent model settings are invalid."
            }),
        ),
        AgentModelSettingsError::DetectionFailed | AgentModelSettingsError::ModelNotDetected => {
            json_facade_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                json!({
                    "error": error.to_string(),
                    "safe_message": "Agent model detection did not find a usable model."
                }),
            )
        }
        AgentModelSettingsError::UpstreamUnauthorized => json_facade_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({
                "error": error.to_string(),
                "safe_message": "Agent model provider rejected the API key."
            }),
        ),
        AgentModelSettingsError::StoreUnavailable
        | AgentModelSettingsError::SecretCryptoFailed
        | AgentModelSettingsError::InvalidStoredProtocol => service_unavailable(
            "agent_settings_unavailable",
            "Agent model settings are temporarily unavailable.",
        ),
    }
}

fn is_stream_route(method: &Method, path: &str) -> bool {
    *method == Method::POST && path == "/agent/stream"
}

fn is_model_catalog_preview_route(method: &Method, path: &str) -> bool {
    *method == Method::POST && path == "/agent/model-catalog/preview"
}

fn is_settings_update_route(method: &Method, path: &str) -> bool {
    *method == Method::PUT && path == "/agent/settings"
}

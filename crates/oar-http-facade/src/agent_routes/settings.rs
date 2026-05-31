use std::sync::Arc;

use http_body_util::BodyExt;
use hyper::body::Incoming;
use hyper::http::{Method, StatusCode};
use serde_json::json;

use crate::agent::{
    decode_agent_model_catalog_request, decode_agent_settings_update_request,
    AgentModelCatalogRequest, AgentRuntime, AgentSettingsSnapshot, AgentSettingsUpdateRequest,
};
use crate::response::{json_facade_response, not_found, service_unavailable, FacadeResponse};
use crate::{
    authenticate_oar_session, oar_session_auth_error_response, AuthenticatedContext,
    OarHttpFacadeRuntime,
};

mod error_response;

use error_response::agent_model_settings_error_response;

pub(super) fn is_body_route(method: &Method, path: &str) -> bool {
    is_model_catalog_preview_route(method, path) || is_settings_update_route(method, path)
}

pub(super) fn is_facade_route(method: &Method, path: &str) -> bool {
    path == "/agent/settings" && (*method == Method::GET || *method == Method::DELETE)
}

pub(super) async fn body_response(
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

    if *method == Method::POST {
        let request = match decode_agent_model_catalog_request(&body) {
            Ok(request) => request,
            Err(error) => return agent_model_settings_error_response(error),
        };
        model_catalog_preview_response(&runtime, &auth_context, request).await
    } else if *method == Method::PUT {
        let request = match decode_agent_settings_update_request(&body) {
            Ok(request) => request,
            Err(error) => return agent_model_settings_error_response(error),
        };
        save_settings_response(&runtime, &auth_context, request).await
    } else {
        not_found()
    }
}

pub(super) async fn facade_response(
    runtime: &OarHttpFacadeRuntime,
    method: &Method,
    path: &str,
    authorization: Option<&str>,
) -> FacadeResponse {
    match path {
        "/agent/settings" if *method == Method::GET => {
            let auth_context = match authenticate_oar_session(runtime, authorization).await {
                Ok(context) => context,
                Err(error) => return oar_session_auth_error_response(error),
            };
            settings_snapshot_response(runtime, &auth_context).await
        }
        "/agent/settings" if *method == Method::DELETE => {
            let auth_context = match authenticate_oar_session(runtime, authorization).await {
                Ok(context) => context,
                Err(error) => return oar_session_auth_error_response(error),
            };
            delete_settings_response(runtime, &auth_context).await
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
        .map(|runtime| AgentSettingsSnapshot::from_summary(runtime.config_summary(), false))
        .unwrap_or_else(|| AgentSettingsSnapshot::missing(false));
    json_facade_response(StatusCode::OK, json!(snapshot))
}

fn is_model_catalog_preview_route(method: &Method, path: &str) -> bool {
    *method == Method::POST && path == "/agent/model-catalog/preview"
}

fn is_settings_update_route(method: &Method, path: &str) -> bool {
    *method == Method::PUT && path == "/agent/settings"
}

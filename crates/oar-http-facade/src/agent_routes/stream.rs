use std::sync::Arc;

use http_body_util::{BodyExt, StreamBody};
use hyper::body::Incoming;
use hyper::header::{CACHE_CONTROL, CONTENT_TYPE};
use hyper::http::{HeaderValue, Method, StatusCode};
use hyper::Response;
use serde_json::json;
use tracing::warn;

use super::body::collect_limited_body;
use crate::agent::{
    decode_agent_stream_request, inject_live_feishu_context, prepend_agent_context_status_frame,
    AgentContextStatus, AgentModelSettingsError, AgentProviderConfig, AgentRequestError,
    AgentRuntime, AgentStreamError,
};
use crate::response::{json_facade_response, service_unavailable, FacadeResponse, ResponseBody};
use crate::{
    authenticate_oar_session, oar_session_auth_error_response, AuthenticatedContext,
    OarHttpFacadeRuntime,
};

const AGENT_STREAM_BODY_LIMIT_BYTES: usize = 1024 * 1024;

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
    let body = match collect_limited_body(
        body,
        AGENT_STREAM_BODY_LIMIT_BYTES,
        "agent_request_body_too_large",
        "Agent request body is too large.",
        "agent_request_body_unreadable",
        "Agent request body could not be read.",
    )
    .await
    {
        Ok(body) => body,
        Err(response) => return response.into_hyper_response(),
    };
    let mut request = match decode_agent_stream_request(&body) {
        Ok(request) => request,
        Err(error) => return agent_request_error_response(error).into_hyper_response(),
    };
    inject_live_feishu_context(&runtime, &auth_context, &mut request).await;
    let context_status = AgentContextStatus::from_request(&request);

    let user_agent_runtime = match user_agent_runtime(&runtime, &auth_context).await {
        UserAgentRuntime::Configured(agent_runtime) => Some(agent_runtime),
        UserAgentRuntime::UseDefault => None,
        UserAgentRuntime::Unavailable => {
            return agent_user_model_settings_unavailable_response().into_hyper_response();
        }
    };
    let agent_runtime = match user_agent_runtime.as_ref().or(runtime.agent.as_deref()) {
        Some(agent_runtime) => agent_runtime,
        None => {
            return service_unavailable(
                "agent_model_not_configured",
                "Agent model provider is not configured in this backend facade.",
            )
            .into_hyper_response();
        }
    };
    let stream = agent_runtime.open_stream(request).await;
    let stream = match stream {
        Ok(stream) => stream,
        Err(error) => return agent_stream_error_response(error).into_hyper_response(),
    };
    let stream = prepend_agent_context_status_frame(stream, context_status);
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

enum UserAgentRuntime {
    Configured(AgentRuntime),
    UseDefault,
    Unavailable,
}

async fn user_agent_runtime(
    runtime: &OarHttpFacadeRuntime,
    auth_context: &AuthenticatedContext,
) -> UserAgentRuntime {
    let Some(settings) = runtime.agent_settings.as_deref() else {
        return UserAgentRuntime::UseDefault;
    };
    user_agent_runtime_from_provider_config(
        settings
            .provider_config_for_user(&auth_context.tenant_id, &auth_context.user_id)
            .await,
    )
}

fn user_agent_runtime_from_provider_config(
    config: Result<Option<AgentProviderConfig>, AgentModelSettingsError>,
) -> UserAgentRuntime {
    let config = match config {
        Ok(Some(config)) => config,
        Ok(None) => return UserAgentRuntime::UseDefault,
        Err(error) => {
            warn!(
                ?error,
                "user agent model settings unavailable; failing closed"
            );
            return UserAgentRuntime::Unavailable;
        }
    };
    match AgentRuntime::from_provider_config(config) {
        Ok(runtime) => UserAgentRuntime::Configured(runtime),
        Err(error) => {
            warn!(?error, "user agent model settings invalid; failing closed");
            UserAgentRuntime::Unavailable
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

fn agent_user_model_settings_unavailable_response() -> FacadeResponse {
    service_unavailable(
        "agent_user_model_settings_unavailable",
        "Agent user model settings are temporarily unavailable.",
    )
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

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    fn auth_context() -> AuthenticatedContext {
        AuthenticatedContext {
            session_id: "oar_session_test".to_string(),
            tenant_id: "tenant-test".to_string(),
            user_id: "user-test".to_string(),
        }
    }

    #[test]
    fn user_agent_runtime_uses_default_only_when_user_settings_are_absent() {
        let resolution = user_agent_runtime_from_provider_config(Ok(None));

        assert!(matches!(resolution, UserAgentRuntime::UseDefault));
    }

    #[test]
    fn user_agent_runtime_fails_closed_when_user_settings_are_unavailable() {
        let resolution = user_agent_runtime_from_provider_config(Err(
            AgentModelSettingsError::SecretCryptoFailed,
        ));

        assert!(matches!(resolution, UserAgentRuntime::Unavailable));
    }

    #[tokio::test]
    async fn user_agent_runtime_uses_default_when_settings_runtime_is_not_configured() {
        let runtime = OarHttpFacadeRuntime::default();

        let resolution = user_agent_runtime(&runtime, &auth_context()).await;

        assert!(matches!(resolution, UserAgentRuntime::UseDefault));
    }

    #[test]
    fn user_model_settings_unavailable_response_is_stable_503_json() {
        let response = agent_user_model_settings_unavailable_response();
        let body = serde_json::from_str::<Value>(&response.body).expect("json body");

        assert_eq!(response.status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.content_type, "application/json");
        assert_eq!(body["error"], "agent_user_model_settings_unavailable");
        assert_eq!(
            body["safe_message"],
            "Agent user model settings are temporarily unavailable."
        );
    }
}

use hyper::http::StatusCode;
use serde_json::json;

use crate::agent::AgentModelSettingsError;
use crate::response::{json_facade_response, service_unavailable, FacadeResponse};

pub(super) fn agent_model_settings_error_response(
    error: AgentModelSettingsError,
) -> FacadeResponse {
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
        | AgentModelSettingsError::InvalidStoredBaseURL
        | AgentModelSettingsError::InvalidStoredProtocol => service_unavailable(
            "agent_settings_unavailable",
            "Agent model settings are temporarily unavailable.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;
    use AgentModelSettingsError::*;

    #[test]
    fn agent_model_settings_errors_map_to_stable_http_responses() {
        let cases = [
            (
                InvalidJson,
                StatusCode::BAD_REQUEST,
                "agent_settings_invalid_json",
            ),
            (
                MissingBaseURL,
                StatusCode::BAD_REQUEST,
                "agent_settings_base_url_required",
            ),
            (
                MissingApiKey,
                StatusCode::BAD_REQUEST,
                "agent_settings_api_key_required",
            ),
            (
                MissingModel,
                StatusCode::BAD_REQUEST,
                "agent_settings_model_required",
            ),
            (
                InvalidBaseURL,
                StatusCode::BAD_REQUEST,
                "agent_settings_base_url_invalid",
            ),
            (
                DetectionFailed,
                StatusCode::UNPROCESSABLE_ENTITY,
                "agent_settings_model_detection_failed",
            ),
            (
                ModelNotDetected,
                StatusCode::UNPROCESSABLE_ENTITY,
                "agent_settings_model_not_detected",
            ),
            (
                UpstreamUnauthorized,
                StatusCode::UNPROCESSABLE_ENTITY,
                "agent_settings_api_key_rejected",
            ),
            (
                StoreUnavailable,
                StatusCode::SERVICE_UNAVAILABLE,
                "agent_settings_unavailable",
            ),
            (
                SecretCryptoFailed,
                StatusCode::SERVICE_UNAVAILABLE,
                "agent_settings_unavailable",
            ),
            (
                InvalidStoredBaseURL,
                StatusCode::SERVICE_UNAVAILABLE,
                "agent_settings_unavailable",
            ),
            (
                InvalidStoredProtocol,
                StatusCode::SERVICE_UNAVAILABLE,
                "agent_settings_unavailable",
            ),
        ];

        for (error, expected_status, expected_code) in cases {
            let response = agent_model_settings_error_response(error);
            let body = serde_json::from_str::<Value>(&response.body).expect("json body");

            assert_eq!(response.status, expected_status);
            assert_eq!(body["error"], expected_code);
        }
    }
}

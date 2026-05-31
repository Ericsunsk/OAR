#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentModelSettingsError {
    InvalidJson,
    MissingBaseURL,
    MissingApiKey,
    MissingModel,
    InvalidBaseURL,
    DetectionFailed,
    UpstreamUnauthorized,
    ModelNotDetected,
    StoreUnavailable,
    SecretCryptoFailed,
    InvalidStoredBaseURL,
    InvalidStoredProtocol,
}

impl std::fmt::Display for AgentModelSettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidJson => write!(f, "agent_settings_invalid_json"),
            Self::MissingBaseURL => write!(f, "agent_settings_base_url_required"),
            Self::MissingApiKey => write!(f, "agent_settings_api_key_required"),
            Self::MissingModel => write!(f, "agent_settings_model_required"),
            Self::InvalidBaseURL => write!(f, "agent_settings_base_url_invalid"),
            Self::DetectionFailed => write!(f, "agent_settings_model_detection_failed"),
            Self::UpstreamUnauthorized => write!(f, "agent_settings_api_key_rejected"),
            Self::ModelNotDetected => write!(f, "agent_settings_model_not_detected"),
            Self::StoreUnavailable => write!(f, "agent_settings_store_unavailable"),
            Self::SecretCryptoFailed => write!(f, "agent_settings_secret_crypto_failed"),
            Self::InvalidStoredBaseURL => write!(f, "agent_settings_stored_base_url_invalid"),
            Self::InvalidStoredProtocol => write!(f, "agent_settings_protocol_invalid"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use AgentModelSettingsError::*;

    #[test]
    fn display_returns_stable_error_codes() {
        let cases = [
            (InvalidJson, "agent_settings_invalid_json"),
            (MissingBaseURL, "agent_settings_base_url_required"),
            (MissingApiKey, "agent_settings_api_key_required"),
            (MissingModel, "agent_settings_model_required"),
            (InvalidBaseURL, "agent_settings_base_url_invalid"),
            (DetectionFailed, "agent_settings_model_detection_failed"),
            (UpstreamUnauthorized, "agent_settings_api_key_rejected"),
            (ModelNotDetected, "agent_settings_model_not_detected"),
            (StoreUnavailable, "agent_settings_store_unavailable"),
            (SecretCryptoFailed, "agent_settings_secret_crypto_failed"),
            (
                InvalidStoredBaseURL,
                "agent_settings_stored_base_url_invalid",
            ),
            (InvalidStoredProtocol, "agent_settings_protocol_invalid"),
        ];

        for (error, expected) in cases {
            assert_eq!(error.to_string(), expected);
        }
    }
}

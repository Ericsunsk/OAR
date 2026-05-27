use super::types::LarkAuthRefreshParseError;

pub(crate) const SAFE_TRANSIENT_ERROR: &str = "temporarily unavailable";
pub(crate) const SAFE_REAUTH_ERROR: &str = "reauthentication required";
pub(crate) const SAFE_CONFIG_ERROR: &str = "refresh_config_required";
pub(crate) const SAFE_PARSE_ERROR: &str = "invalid lark auth refresh envelope";

pub(crate) fn sanitize_safe_error(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() || contains_sensitive_marker(trimmed) {
        return fallback.to_string();
    }

    match trimmed {
        "invalid_grant" | SAFE_TRANSIENT_ERROR | SAFE_REAUTH_ERROR | SAFE_CONFIG_ERROR => {
            trimmed.to_string()
        }
        _ => fallback.to_string(),
    }
}

pub(crate) fn reject_sensitive_json(
    value: &serde_json::Value,
) -> Result<(), LarkAuthRefreshParseError> {
    match value {
        serde_json::Value::Object(map) => {
            for (key, nested) in map {
                if contains_sensitive_marker(key) {
                    return Err(LarkAuthRefreshParseError::SensitiveContentDetected);
                }
                reject_sensitive_json(nested)?;
            }
            Ok(())
        }
        serde_json::Value::Array(items) => {
            for item in items {
                reject_sensitive_json(item)?;
            }
            Ok(())
        }
        serde_json::Value::String(text) => {
            if contains_sensitive_marker(text) {
                Err(LarkAuthRefreshParseError::SensitiveContentDetected)
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }
}

pub(crate) fn contains_sensitive_marker(input: &str) -> bool {
    let normalized = input.to_ascii_lowercase();
    normalized.contains("access_token")
        || normalized.contains("refresh_token")
        || normalized.contains("authorization_code")
        || normalized.contains("auth_code")
        || normalized.contains("authorization")
        || normalized.contains("bearer")
        || normalized.contains("refresh_token=")
}

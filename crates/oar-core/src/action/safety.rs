const SAFE_ADAPTER_ERROR_MESSAGE: &str = "adapter execution failed";
const FALLBACK_ADAPTER_ERROR_CODE: &str = "adapter_error";
const MAX_SAFE_ERROR_MESSAGE_CHARS: usize = 240;
const MAX_ERROR_CODE_CHARS: usize = 64;

pub(crate) fn sanitize_adapter_error_message(message: &str) -> String {
    let trimmed = message.trim();
    if trimmed.is_empty() || contains_sensitive_marker(trimmed) {
        return SAFE_ADAPTER_ERROR_MESSAGE.to_string();
    }

    let normalized: String = trimmed
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(MAX_SAFE_ERROR_MESSAGE_CHARS)
        .collect();

    if normalized.trim().is_empty() || contains_sensitive_marker(&normalized) {
        SAFE_ADAPTER_ERROR_MESSAGE.to_string()
    } else {
        normalized
    }
}

pub(crate) fn sanitize_adapter_error_code(code: &str) -> String {
    let trimmed = code.trim();
    if trimmed.is_empty() {
        return FALLBACK_ADAPTER_ERROR_CODE.to_string();
    }

    let normalized: String = trimmed
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .take(MAX_ERROR_CODE_CHARS)
        .collect();

    if normalized.is_empty() {
        FALLBACK_ADAPTER_ERROR_CODE.to_string()
    } else {
        normalized
    }
}

fn contains_sensitive_marker(input: &str) -> bool {
    let lowered = input.to_ascii_lowercase();
    [
        "access token",
        "access_token",
        "refresh token",
        "refresh_token",
        "authorization:",
        "authorization code",
        "authorization_code",
        "auth code",
        "auth_code",
        "bearer ",
        "client_secret",
        "oauth_grant",
        "stdout",
        "stderr",
    ]
    .iter()
    .any(|needle| lowered.contains(needle))
}

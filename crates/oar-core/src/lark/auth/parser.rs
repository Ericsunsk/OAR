use super::safety::{contains_sensitive_marker, reject_sensitive_json};
use super::types::{
    FeishuAuthRefreshFailure, FeishuAuthRefreshParseError, FeishuAuthRefreshResponse,
    FeishuAuthRefreshSuccess,
};

pub fn parse_feishu_auth_refresh_response(
    raw: &str,
) -> Result<FeishuAuthRefreshResponse, FeishuAuthRefreshParseError> {
    if contains_sensitive_marker(raw) {
        return Err(FeishuAuthRefreshParseError::SensitiveContentDetected);
    }

    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|_| FeishuAuthRefreshParseError::InvalidEnvelope)?;
    reject_sensitive_json(&value)?;

    parse_safe_envelope(value)
}

fn parse_safe_envelope(
    value: serde_json::Value,
) -> Result<FeishuAuthRefreshResponse, FeishuAuthRefreshParseError> {
    let obj = value
        .as_object()
        .ok_or(FeishuAuthRefreshParseError::InvalidEnvelope)?;
    let outcome = obj
        .get("outcome")
        .and_then(serde_json::Value::as_str)
        .ok_or(FeishuAuthRefreshParseError::InvalidEnvelope)?;

    match outcome {
        "success" => {
            let encrypted_primary = parse_byte_vec(obj.get("encrypted_primary"))?;
            let encrypted_renewal = parse_byte_vec(obj.get("encrypted_renewal"))?;
            let key_id = parse_string(obj.get("key_id"))?;
            let new_fingerprint = parse_string(obj.get("new_fingerprint"))?;
            let refreshed_at_ms = parse_u64(obj.get("refreshed_at_ms"))?;
            let expires_at_ms = match obj.get("expires_at_ms") {
                Some(serde_json::Value::Null) | None => None,
                Some(value) => Some(parse_u64(Some(value))?),
            };
            Ok(FeishuAuthRefreshResponse::Success(
                FeishuAuthRefreshSuccess {
                    encrypted_primary,
                    encrypted_renewal,
                    key_id,
                    new_fingerprint,
                    refreshed_at_ms,
                    expires_at_ms,
                },
            ))
        }
        "transient_failure" => Ok(FeishuAuthRefreshResponse::Failure(
            FeishuAuthRefreshFailure::Transient {
                safe_error: parse_string(obj.get("safe_error"))?,
            },
        )),
        "reauth_required" => Ok(FeishuAuthRefreshResponse::Failure(
            FeishuAuthRefreshFailure::ReauthRequired {
                safe_error: parse_string(obj.get("safe_error"))?,
            },
        )),
        "config_required" => Ok(FeishuAuthRefreshResponse::Failure(
            FeishuAuthRefreshFailure::ConfigRequired {
                safe_error: parse_string(obj.get("safe_error"))?,
            },
        )),
        _ => Err(FeishuAuthRefreshParseError::InvalidEnvelope),
    }
}

fn parse_string(value: Option<&serde_json::Value>) -> Result<String, FeishuAuthRefreshParseError> {
    value
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or(FeishuAuthRefreshParseError::InvalidEnvelope)
}

fn parse_u64(value: Option<&serde_json::Value>) -> Result<u64, FeishuAuthRefreshParseError> {
    value
        .and_then(serde_json::Value::as_u64)
        .ok_or(FeishuAuthRefreshParseError::InvalidEnvelope)
}

fn parse_byte_vec(
    value: Option<&serde_json::Value>,
) -> Result<Vec<u8>, FeishuAuthRefreshParseError> {
    let arr = value
        .and_then(serde_json::Value::as_array)
        .ok_or(FeishuAuthRefreshParseError::InvalidEnvelope)?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let n = item
            .as_u64()
            .ok_or(FeishuAuthRefreshParseError::InvalidEnvelope)?;
        let byte = u8::try_from(n).map_err(|_| FeishuAuthRefreshParseError::InvalidEnvelope)?;
        out.push(byte);
    }
    Ok(out)
}

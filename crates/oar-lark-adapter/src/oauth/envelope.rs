use oar_core::lark::auth::client::LarkAuthRefreshRawEnvelope;
use serde_json::{json, Value};

use super::transport::FeishuOAuthTransportError;
use super::types::FeishuGrantEnvelope;

pub(super) fn raw_envelope(
    value: Value,
) -> Result<LarkAuthRefreshRawEnvelope, FeishuOAuthTransportError> {
    serde_json::to_string(&value)
        .map(LarkAuthRefreshRawEnvelope::new)
        .map_err(|_| FeishuOAuthTransportError::EnvelopeSerializationFailed)
}

pub(super) fn success_envelope_value(envelope: FeishuGrantEnvelope) -> Value {
    json!({
        "outcome": "success",
        "encrypted_primary": envelope.encrypted_primary,
        "encrypted_renewal": envelope.encrypted_renewal,
        "key_id": envelope.key_id,
        "new_fingerprint": envelope.new_fingerprint,
        "refreshed_at_ms": envelope.refreshed_at_ms,
        "expires_at_ms": envelope.expires_at_ms,
    })
}

pub(super) fn failure_envelope_value(outcome: &str) -> Value {
    let safe_error = match outcome {
        "reauth_required" => "invalid_grant",
        "config_required" => "refresh_config_required",
        _ => "temporarily unavailable",
    };
    failure_envelope_value_for_safe_error(outcome, safe_error)
}

pub(super) fn failure_envelope_value_for_safe_error(outcome: &str, safe_error: &str) -> Value {
    json!({
        "outcome": outcome,
        "safe_error": safe_error,
    })
}

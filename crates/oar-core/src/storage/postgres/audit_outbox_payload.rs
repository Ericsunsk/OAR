use serde_json::Value;

const MAX_AUDIT_OUTBOX_PAYLOAD_TEXT_LEN: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuditOutboxPayloadSafetyError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeAuditOutboxPayload {
    pub event_id: Option<String>,
    pub trace_id: Option<String>,
    pub event_type: Option<String>,
    pub sequence: Option<u64>,
    pub tenant_id: Option<String>,
    pub kind: Option<String>,
}

impl TryFrom<&Value> for SafeAuditOutboxPayload {
    type Error = AuditOutboxPayloadSafetyError;

    fn try_from(payload: &Value) -> Result<Self, Self::Error> {
        let object = payload.as_object().ok_or(AuditOutboxPayloadSafetyError)?;
        if object.is_empty() {
            return Err(AuditOutboxPayloadSafetyError);
        }

        let mut safe = Self {
            event_id: None,
            trace_id: None,
            event_type: None,
            sequence: None,
            tenant_id: None,
            kind: None,
        };

        for (key, value) in object {
            match key.as_str() {
                "event_id" => {
                    safe.event_id = Some(validate_audit_outbox_payload_text(value)?.to_string());
                }
                "trace_id" => {
                    safe.trace_id = Some(validate_audit_outbox_payload_text(value)?.to_string());
                }
                "event_type" => {
                    safe.event_type = Some(validate_audit_outbox_payload_text(value)?.to_string());
                }
                "tenant_id" => {
                    safe.tenant_id = Some(validate_audit_outbox_payload_text(value)?.to_string());
                }
                "kind" => {
                    safe.kind = Some(validate_audit_outbox_payload_text(value)?.to_string());
                }
                "sequence" => {
                    safe.sequence = Some(value.as_u64().ok_or(AuditOutboxPayloadSafetyError)?);
                }
                _ => return Err(AuditOutboxPayloadSafetyError),
            }
        }

        Ok(safe)
    }
}

pub fn validate_audit_outbox_payload(payload: &Value) -> Result<(), AuditOutboxPayloadSafetyError> {
    SafeAuditOutboxPayload::try_from(payload).map(|_| ())
}

fn validate_audit_outbox_payload_text(
    value: &Value,
) -> Result<&str, AuditOutboxPayloadSafetyError> {
    let text = value.as_str().ok_or(AuditOutboxPayloadSafetyError)?;
    validate_audit_outbox_text(text)?;
    Ok(text)
}

pub fn validate_audit_outbox_text(value: &str) -> Result<(), AuditOutboxPayloadSafetyError> {
    if value.trim().is_empty()
        || value.len() > MAX_AUDIT_OUTBOX_PAYLOAD_TEXT_LEN
        || contains_sensitive_audit_outbox_marker(value)
    {
        return Err(AuditOutboxPayloadSafetyError);
    }
    Ok(())
}

fn contains_sensitive_audit_outbox_marker(value: &str) -> bool {
    let lowered = value.to_ascii_lowercase();
    contains_sensitive_audit_outbox_direct_marker(&lowered)
        || lowered.contains("encrypted")
        || lowered.contains("fingerprint")
}

fn contains_sensitive_audit_outbox_direct_marker(value: &str) -> bool {
    [
        "access token",
        "access_token",
        "accesstoken",
        "refresh token",
        "refresh_token",
        "refreshtoken",
        "authorization:",
        "authorization code",
        "authorization_code",
        "authorization-code",
        "authorizationcode",
        "auth code",
        "auth_code",
        "auth-code",
        "bearer ",
        "client_secret",
        "oauth_grant",
        "stdout",
        "stderr",
    ]
    .iter()
    .any(|needle| value.contains(needle))
        || contains_sensitive_audit_outbox_segments(value)
}

fn contains_sensitive_audit_outbox_segments(value: &str) -> bool {
    let segments = value
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    for (index, segment) in segments.iter().enumerate() {
        if matches!(*segment, "secret" | "password" | "credential") {
            return true;
        }
        if *segment == "token" && segments.get(index + 1).copied() != Some("refresh") {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_outbox_payload_accepts_minimal_route_identifiers() {
        assert!(validate_audit_outbox_payload(&serde_json::json!({
            "trace_id": "trace_token_refresh_sweep_success",
            "kind": "token_refresh_sweep",
            "sequence": 1
        }))
        .is_ok());
    }

    #[test]
    fn audit_outbox_payload_rejects_sensitive_markers_without_payload_echo() {
        let error = validate_audit_outbox_payload(&serde_json::json!({
            "trace_id": "access token tok_secret",
        }))
        .expect_err("sensitive payload must fail closed");

        let rendered = format!("{error:?}");
        assert!(!rendered.contains("tok_secret"));
        assert!(!rendered.contains("access token"));
    }

    #[test]
    fn audit_outbox_payload_rejects_unknown_fields_and_wrong_types() {
        assert!(validate_audit_outbox_payload(&serde_json::json!({
            "trace_id": "trace_1",
            "raw_stdout": "ok"
        }))
        .is_err());
        assert!(validate_audit_outbox_payload(&serde_json::json!({
            "trace_id": true
        }))
        .is_err());
        assert!(validate_audit_outbox_payload(&serde_json::json!({
            "sequence": "1"
        }))
        .is_err());
    }
}

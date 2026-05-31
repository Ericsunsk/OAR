use std::fmt;

use oar_core::storage::postgres::{
    validate_audit_outbox_text, AuditOutboxMessage, SafeAuditOutboxPayload,
};
use serde::Serialize;
use serde_json::Value;

use super::sink::{unsafe_envelope_error, AuditOutboxSinkError};

#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct AuditOutboxDeliveryEnvelope {
    pub delivery_id: String,
    pub tenant_id: String,
    pub stream: String,
    pub aggregate_id: String,
    pub payload: AuditOutboxSafePayload,
    pub attempt_count: i32,
}

impl fmt::Debug for AuditOutboxDeliveryEnvelope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditOutboxDeliveryEnvelope")
            .field("delivery_id", &self.delivery_id)
            .field("tenant_id", &self.tenant_id)
            .field("stream", &self.stream)
            .field("aggregate_id", &self.aggregate_id)
            .field("payload", &self.payload)
            .field("attempt_count", &self.attempt_count)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuditOutboxSafePayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sequence: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

impl TryFrom<&AuditOutboxMessage> for AuditOutboxDeliveryEnvelope {
    type Error = AuditOutboxSinkError;

    fn try_from(message: &AuditOutboxMessage) -> Result<Self, Self::Error> {
        validate_text(&message.tenant_id)?;
        validate_text(&message.stream)?;
        validate_text(&message.aggregate_id)?;
        let payload = AuditOutboxSafePayload::try_from(&message.payload)?;
        Ok(Self {
            delivery_id: stable_delivery_id(message),
            tenant_id: message.tenant_id.clone(),
            stream: message.stream.clone(),
            aggregate_id: message.aggregate_id.clone(),
            payload,
            attempt_count: message.attempt_count,
        })
    }
}

impl TryFrom<&Value> for AuditOutboxSafePayload {
    type Error = AuditOutboxSinkError;

    fn try_from(payload: &Value) -> Result<Self, Self::Error> {
        SafeAuditOutboxPayload::try_from(payload)
            .map(AuditOutboxSafePayload::from)
            .map_err(|_| unsafe_envelope_error())
    }
}

impl From<SafeAuditOutboxPayload> for AuditOutboxSafePayload {
    fn from(payload: SafeAuditOutboxPayload) -> Self {
        Self {
            event_id: payload.event_id,
            trace_id: payload.trace_id,
            event_type: payload.event_type,
            sequence: payload.sequence,
            tenant_id: payload.tenant_id,
            kind: payload.kind,
        }
    }
}

fn stable_delivery_id(message: &AuditOutboxMessage) -> String {
    format!(
        "{}:{}:{}:{}",
        message.tenant_id, message.stream, message.aggregate_id, message.id
    )
}

fn validate_text(value: &str) -> Result<(), AuditOutboxSinkError> {
    validate_audit_outbox_text(value).map_err(|_| unsafe_envelope_error())
}

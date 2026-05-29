use std::{convert::Infallible, fmt};

use async_trait::async_trait;
use oar_core::storage::postgres::audit_outbox_worker::{
    AuditOutboxDelivery, AuditOutboxDispatcher,
};
use oar_core::storage::postgres::{
    validate_audit_outbox_text, AuditOutboxMessage, SafeAuditOutboxPayload,
};
use serde_json::Value;

const SAFE_ERROR_RETRYABLE: &str = "audit_outbox_sink_retryable";
const SAFE_ERROR_FAILED: &str = "audit_outbox_sink_failed";
const SAFE_ERROR_UNSAFE_ENVELOPE: &str = "audit_outbox_unsafe_envelope";
const SAFE_ERROR_SINK_UNAVAILABLE: &str = "audit_outbox_sink_unavailable";

#[derive(Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditOutboxSafePayload {
    pub event_id: Option<String>,
    pub trace_id: Option<String>,
    pub event_type: Option<String>,
    pub sequence: Option<u64>,
    pub tenant_id: Option<String>,
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditOutboxSinkDelivery {
    Sent,
    Retryable,
    Failed,
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditOutboxSinkError {
    Retryable { safe_error: &'static str },
    Failed { safe_error: &'static str },
}

impl AuditOutboxSinkError {
    pub fn retryable() -> Self {
        Self::Retryable {
            safe_error: SAFE_ERROR_RETRYABLE,
        }
    }

    pub fn failed() -> Self {
        Self::Failed {
            safe_error: SAFE_ERROR_FAILED,
        }
    }

    pub fn safe_error(&self) -> &'static str {
        match self {
            Self::Retryable { safe_error } | Self::Failed { safe_error } => safe_error,
        }
    }

    pub fn classify(&self) -> AuditOutboxSinkDelivery {
        match self {
            Self::Retryable { .. } => AuditOutboxSinkDelivery::Retryable,
            Self::Failed { .. } => AuditOutboxSinkDelivery::Failed,
        }
    }
}

impl fmt::Debug for AuditOutboxSinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditOutboxSinkError")
            .field("classification", &self.classify())
            .field("safe_error", &self.safe_error())
            .finish()
    }
}

impl fmt::Display for AuditOutboxSinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.safe_error())
    }
}

impl std::error::Error for AuditOutboxSinkError {}

#[async_trait]
pub trait AuditOutboxSink {
    async fn deliver(
        &mut self,
        envelope: AuditOutboxDeliveryEnvelope,
    ) -> Result<AuditOutboxSinkDelivery, AuditOutboxSinkError>;
}

pub struct AuditOutboxSinkDispatcher<S> {
    sink: S,
}

impl<S> AuditOutboxSinkDispatcher<S> {
    pub fn new(sink: S) -> Self {
        Self { sink }
    }

    pub fn sink(&self) -> &S {
        &self.sink
    }
}

impl<S> fmt::Debug for AuditOutboxSinkDispatcher<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditOutboxSinkDispatcher")
            .field("sink", &"[REDACTED]")
            .finish()
    }
}

impl<S> AuditOutboxDispatcher for AuditOutboxSinkDispatcher<S>
where
    S: AuditOutboxSink + Send,
{
    type Error = Infallible;

    async fn deliver(
        &mut self,
        message: &AuditOutboxMessage,
    ) -> Result<AuditOutboxDelivery, Self::Error> {
        let delivery = match AuditOutboxDeliveryEnvelope::try_from(message) {
            Ok(envelope) => self
                .sink
                .deliver(envelope)
                .await
                .unwrap_or_else(|error| error.classify()),
            Err(error) => error.classify(),
        };
        Ok(core_delivery(delivery))
    }
}

#[derive(Default)]
pub struct NoopAuditOutboxSink {
    delivered: Vec<AuditOutboxDeliveryEnvelope>,
}

impl NoopAuditOutboxSink {
    pub fn delivered(&self) -> &[AuditOutboxDeliveryEnvelope] {
        &self.delivered
    }
}

impl fmt::Debug for NoopAuditOutboxSink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NoopAuditOutboxSink")
            .field("delivered_count", &self.delivered.len())
            .finish()
    }
}

#[async_trait]
impl AuditOutboxSink for NoopAuditOutboxSink {
    async fn deliver(
        &mut self,
        envelope: AuditOutboxDeliveryEnvelope,
    ) -> Result<AuditOutboxSinkDelivery, AuditOutboxSinkError> {
        self.delivered.push(envelope);
        Ok(AuditOutboxSinkDelivery::Sent)
    }
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

fn core_delivery(delivery: AuditOutboxSinkDelivery) -> AuditOutboxDelivery {
    match delivery {
        AuditOutboxSinkDelivery::Sent => AuditOutboxDelivery::Sent,
        AuditOutboxSinkDelivery::Retryable => AuditOutboxDelivery::Retryable,
        AuditOutboxSinkDelivery::Failed => AuditOutboxDelivery::Failed,
    }
}

fn unsafe_envelope_error() -> AuditOutboxSinkError {
    AuditOutboxSinkError::Failed {
        safe_error: SAFE_ERROR_UNSAFE_ENVELOPE,
    }
}

pub fn sink_unavailable_error() -> AuditOutboxSinkError {
    AuditOutboxSinkError::Retryable {
        safe_error: SAFE_ERROR_SINK_UNAVAILABLE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(payload: Value) -> AuditOutboxMessage {
        AuditOutboxMessage {
            id: 42,
            tenant_id: "tenant_1".to_string(),
            stream: "audit-events".to_string(),
            aggregate_id: "trace_1".to_string(),
            payload,
            attempt_count: 3,
            next_attempt_at_ms: Some(123),
        }
    }

    #[test]
    fn envelope_accepts_minimal_safe_payload_and_builds_stable_delivery_id() {
        let envelope = AuditOutboxDeliveryEnvelope::try_from(&message(serde_json::json!({
            "event_id": "evt_1",
            "trace_id": "trace_1",
            "event_type": "execution_succeeded",
            "sequence": 7,
            "tenant_id": "tenant_1",
            "kind": "audit_event"
        })))
        .expect("safe outbox envelope");

        assert_eq!(envelope.delivery_id, "tenant_1:audit-events:trace_1:42");
        assert_eq!(envelope.payload.event_id.as_deref(), Some("evt_1"));
        assert_eq!(envelope.payload.sequence, Some(7));
    }

    #[test]
    fn envelope_accepts_token_refresh_as_business_route_identifier() {
        let envelope = AuditOutboxDeliveryEnvelope::try_from(&message(serde_json::json!({
            "trace_id": "trace_token_refresh_sweep_success",
            "event_type": "execution_succeeded",
            "kind": "token_refresh_sweep"
        })))
        .expect("token_refresh is a route identifier, not raw token material");

        assert_eq!(
            envelope.payload.trace_id.as_deref(),
            Some("trace_token_refresh_sweep_success")
        );
        assert_eq!(
            envelope.payload.kind.as_deref(),
            Some("token_refresh_sweep")
        );
    }

    #[test]
    fn envelope_rejects_sensitive_or_unknown_payload_without_echoing_secret() {
        for payload in [
            serde_json::json!({ "trace_id": "access_token=tok_secret" }),
            serde_json::json!({ "trace_id": "access token tok_secret" }),
            serde_json::json!({ "trace_id": "token=tok_secret" }),
            serde_json::json!({ "encrypted": "blob" }),
            serde_json::json!({ "trace_id": { "nested": true } }),
            serde_json::json!({ "unknown": "value" }),
        ] {
            let error = AuditOutboxDeliveryEnvelope::try_from(&message(payload))
                .expect_err("unsafe payload must fail closed");
            assert_eq!(error.classify(), AuditOutboxSinkDelivery::Failed);
            assert_eq!(error.safe_error(), SAFE_ERROR_UNSAFE_ENVELOPE);
            let rendered = format!("{error:?} {error}");
            assert!(!rendered.contains("tok_secret"));
            assert!(!rendered.contains("access_token"));
            assert!(!rendered.contains("encrypted"));
        }
    }

    #[test]
    fn dispatcher_maps_sink_delivery_to_core_delivery() {
        struct FixedSink(AuditOutboxSinkDelivery);

        #[async_trait]
        impl AuditOutboxSink for FixedSink {
            async fn deliver(
                &mut self,
                _envelope: AuditOutboxDeliveryEnvelope,
            ) -> Result<AuditOutboxSinkDelivery, AuditOutboxSinkError> {
                Ok(self.0)
            }
        }

        let mut dispatcher =
            AuditOutboxSinkDispatcher::new(FixedSink(AuditOutboxSinkDelivery::Sent));
        let delivery = runtime().block_on(async {
            dispatcher
                .deliver(&message(serde_json::json!({ "trace_id": "trace_1" })))
                .await
        });
        let delivery = delivery.expect("delivery should succeed");
        assert_eq!(delivery, AuditOutboxDelivery::Sent);
    }

    #[test]
    fn dispatcher_maps_sink_error_classification_without_retryable_fallback() {
        struct ErrorSink(AuditOutboxSinkError);

        #[async_trait]
        impl AuditOutboxSink for ErrorSink {
            async fn deliver(
                &mut self,
                _envelope: AuditOutboxDeliveryEnvelope,
            ) -> Result<AuditOutboxSinkDelivery, AuditOutboxSinkError> {
                Err(self.0.clone())
            }
        }

        let mut failed_dispatcher =
            AuditOutboxSinkDispatcher::new(ErrorSink(AuditOutboxSinkError::failed()));
        let failed = runtime().block_on(async {
            failed_dispatcher
                .deliver(&message(serde_json::json!({ "trace_id": "trace_1" })))
                .await
        });
        assert_eq!(
            failed.expect("dispatcher is infallible"),
            AuditOutboxDelivery::Failed
        );

        let mut retryable_dispatcher =
            AuditOutboxSinkDispatcher::new(ErrorSink(AuditOutboxSinkError::retryable()));
        let retryable = runtime().block_on(async {
            retryable_dispatcher
                .deliver(&message(serde_json::json!({ "trace_id": "trace_1" })))
                .await
        });
        assert_eq!(
            retryable.expect("dispatcher is infallible"),
            AuditOutboxDelivery::Retryable
        );
    }

    #[test]
    fn dispatcher_fails_unsafe_envelope_without_calling_sink() {
        struct PanicSink;

        #[async_trait]
        impl AuditOutboxSink for PanicSink {
            async fn deliver(
                &mut self,
                _envelope: AuditOutboxDeliveryEnvelope,
            ) -> Result<AuditOutboxSinkDelivery, AuditOutboxSinkError> {
                panic!("unsafe envelope must not reach sink");
            }
        }

        let mut dispatcher = AuditOutboxSinkDispatcher::new(PanicSink);
        let delivery = runtime().block_on(async {
            dispatcher
                .deliver(&message(serde_json::json!({
                    "trace_id": "access_token=tok_secret"
                })))
                .await
        });
        assert_eq!(
            delivery.expect("dispatcher is infallible"),
            AuditOutboxDelivery::Failed
        );
    }

    #[test]
    fn noop_sink_records_safe_envelope_without_external_write() {
        let mut sink = NoopAuditOutboxSink::default();
        let delivery = runtime().block_on(async {
            sink.deliver(
                AuditOutboxDeliveryEnvelope::try_from(&message(serde_json::json!({
                    "trace_id": "trace_1"
                })))
                .expect("safe outbox envelope"),
            )
            .await
        });
        let delivery = delivery.expect("noop sink should accept envelope");

        assert_eq!(delivery, AuditOutboxSinkDelivery::Sent);
        assert_eq!(sink.delivered().len(), 1);
        let debug = format!("{sink:?}");
        assert!(debug.contains("delivered_count"));
        assert!(!debug.contains("trace_1"));
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("test tokio runtime should build")
    }
}

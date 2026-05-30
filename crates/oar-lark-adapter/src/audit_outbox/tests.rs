use async_trait::async_trait;
use oar_core::storage::postgres::audit_outbox_worker::{
    AuditOutboxDelivery, AuditOutboxDispatcher,
};
use oar_core::storage::postgres::AuditOutboxMessage;
use serde_json::Value;

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
        assert_eq!(error.safe_error(), "audit_outbox_unsafe_envelope");
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

    let mut dispatcher = AuditOutboxSinkDispatcher::new(FixedSink(AuditOutboxSinkDelivery::Sent));
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

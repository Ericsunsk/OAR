use async_trait::async_trait;
use oar_core::storage::postgres::audit_outbox_worker::{
    AuditOutboxDelivery, AuditOutboxDispatcher,
};
use oar_core::storage::postgres::AuditOutboxMessage;
use serde_json::Value;

use super::*;
use crate::oauth::{AsyncHttpClient, HttpClientFailure, HttpRequest, HttpResponse};

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

#[test]
fn webhook_sink_posts_safe_envelope_with_idempotency_headers() {
    let http_client = RecordingAsyncHttpClient::from_response(HttpResponse::new(202, "{}"));
    let mut sink = WebhookAuditOutboxSink::with_max_response_bytes(
        "https://audit.example.test/webhook?token=webhook-secret",
        http_client,
        1024,
    )
    .expect("webhook sink should build");
    let delivery = runtime().block_on(async {
        sink.deliver(
            AuditOutboxDeliveryEnvelope::try_from(&message(serde_json::json!({
                "trace_id": "trace_1",
                "kind": "audit_event"
            })))
            .expect("safe envelope"),
        )
        .await
    });
    assert_eq!(
        delivery.expect("webhook delivery should succeed"),
        AuditOutboxSinkDelivery::Sent
    );

    let request = sink
        .http_client()
        .requests
        .first()
        .expect("webhook request");
    assert_eq!(request.method, "POST");
    assert_eq!(
        request.url,
        "https://audit.example.test/webhook?token=webhook-secret"
    );
    assert_eq!(request.max_response_bytes, 1024);
    assert_eq!(
        header_value(&request.headers, "Idempotency-Key"),
        Some("tenant_1:audit-events:trace_1:42")
    );
    assert_eq!(
        header_value(&request.headers, "X-OAR-Delivery-ID"),
        Some("tenant_1:audit-events:trace_1:42")
    );
    assert_eq!(
        header_value(&request.headers, "X-OAR-Tenant-ID"),
        Some("tenant_1")
    );
    assert_eq!(
        request.body["delivery_id"],
        serde_json::json!("tenant_1:audit-events:trace_1:42")
    );
    assert_eq!(
        request.body["payload"]["trace_id"],
        serde_json::json!("trace_1")
    );
    assert_eq!(
        request.body["payload"]["kind"],
        serde_json::json!("audit_event")
    );

    let rendered = format!("{sink:?} {request:?}");
    assert!(!rendered.contains("webhook-secret"));
    assert!(!rendered.contains("token="));
}

#[test]
fn webhook_sink_classifies_status_and_transport_safely() {
    for (status, expected) in [
        (200, AuditOutboxSinkDelivery::Sent),
        (299, AuditOutboxSinkDelivery::Sent),
        (408, AuditOutboxSinkDelivery::Retryable),
        (429, AuditOutboxSinkDelivery::Retryable),
        (503, AuditOutboxSinkDelivery::Retryable),
        (400, AuditOutboxSinkDelivery::Failed),
        (401, AuditOutboxSinkDelivery::Failed),
    ] {
        let delivery = deliver_with_webhook_result(Ok(HttpResponse::new(status, "{}")))
            .expect("status classification should not error");
        assert_eq!(delivery, expected, "status {status}");
    }

    let error = deliver_with_webhook_result(Err(HttpClientFailure::Transport))
        .expect_err("transport should map to retryable sink error");
    assert_eq!(error.classify(), AuditOutboxSinkDelivery::Retryable);
    let rendered = format!("{error:?} {error}");
    assert!(!rendered.contains("webhook-secret"));
}

#[test]
fn webhook_sink_rejects_invalid_config_without_echoing_endpoint() {
    for endpoint in [
        "http://audit.example.test/webhook?token=webhook-secret",
        "not-a-url?token=webhook-secret",
    ] {
        let error = WebhookAuditOutboxSink::new(
            endpoint,
            RecordingAsyncHttpClient::from_response(HttpResponse::new(200, "{}")),
        )
        .expect_err("invalid endpoint should fail");
        let rendered = format!("{error:?} {error}");
        assert!(!rendered.contains("webhook-secret"));
        assert!(!rendered.contains(endpoint));
    }

    let error = WebhookAuditOutboxSink::with_max_response_bytes(
        "https://audit.example.test/webhook?token=webhook-secret",
        RecordingAsyncHttpClient::from_response(HttpResponse::new(200, "{}")),
        0,
    )
    .expect_err("zero max response bytes should fail");
    let rendered = format!("{error:?} {error}");
    assert!(!rendered.contains("webhook-secret"));
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("test tokio runtime should build")
}

fn deliver_with_webhook_result(
    result: Result<HttpResponse, HttpClientFailure>,
) -> Result<AuditOutboxSinkDelivery, AuditOutboxSinkError> {
    let mut sink = WebhookAuditOutboxSink::new(
        "https://audit.example.test/webhook?token=webhook-secret",
        RecordingAsyncHttpClient::from_result(result),
    )
    .expect("webhook sink should build");
    runtime().block_on(async {
        sink.deliver(
            AuditOutboxDeliveryEnvelope::try_from(&message(serde_json::json!({
                "trace_id": "trace_1"
            })))
            .expect("safe envelope"),
        )
        .await
    })
}

fn header_value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(header_name, _)| header_name == name)
        .map(|(_, value)| value.as_str())
}

#[derive(Debug)]
struct RecordingAsyncHttpClient {
    result: Result<HttpResponse, HttpClientFailure>,
    requests: Vec<HttpRequest>,
}

impl RecordingAsyncHttpClient {
    fn from_response(response: HttpResponse) -> Self {
        Self::from_result(Ok(response))
    }

    fn from_result(result: Result<HttpResponse, HttpClientFailure>) -> Self {
        Self {
            result,
            requests: Vec::new(),
        }
    }
}

#[async_trait]
impl AsyncHttpClient for RecordingAsyncHttpClient {
    async fn post_json(&mut self, request: HttpRequest) -> Result<HttpResponse, HttpClientFailure> {
        self.requests.push(request);
        self.result.clone()
    }
}

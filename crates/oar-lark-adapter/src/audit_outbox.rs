mod dispatcher;
mod envelope;
mod noop;
mod sink;
mod webhook;

pub use dispatcher::AuditOutboxSinkDispatcher;
pub use envelope::{AuditOutboxDeliveryEnvelope, AuditOutboxSafePayload};
pub use noop::NoopAuditOutboxSink;
pub use sink::{
    sink_unavailable_error, AuditOutboxSink, AuditOutboxSinkDelivery, AuditOutboxSinkError,
};
pub use webhook::{WebhookAuditOutboxSink, WebhookAuditOutboxSinkConfigError};

#[cfg(test)]
mod tests;

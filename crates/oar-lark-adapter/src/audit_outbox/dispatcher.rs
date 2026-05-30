use std::{convert::Infallible, fmt};

use oar_core::storage::postgres::audit_outbox_worker::{
    AuditOutboxDelivery, AuditOutboxDispatcher,
};
use oar_core::storage::postgres::AuditOutboxMessage;

use super::{AuditOutboxDeliveryEnvelope, AuditOutboxSink, AuditOutboxSinkDelivery};

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

fn core_delivery(delivery: AuditOutboxSinkDelivery) -> AuditOutboxDelivery {
    match delivery {
        AuditOutboxSinkDelivery::Sent => AuditOutboxDelivery::Sent,
        AuditOutboxSinkDelivery::Retryable => AuditOutboxDelivery::Retryable,
        AuditOutboxSinkDelivery::Failed => AuditOutboxDelivery::Failed,
    }
}

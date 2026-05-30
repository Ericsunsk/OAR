use std::fmt;

use async_trait::async_trait;

use super::{
    AuditOutboxDeliveryEnvelope, AuditOutboxSink, AuditOutboxSinkDelivery, AuditOutboxSinkError,
};

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

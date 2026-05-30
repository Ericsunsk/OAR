use std::fmt;

use async_trait::async_trait;

use super::envelope::AuditOutboxDeliveryEnvelope;

const SAFE_ERROR_RETRYABLE: &str = "audit_outbox_sink_retryable";
const SAFE_ERROR_FAILED: &str = "audit_outbox_sink_failed";
const SAFE_ERROR_SINK_UNAVAILABLE: &str = "audit_outbox_sink_unavailable";
const SAFE_ERROR_UNSAFE_ENVELOPE: &str = "audit_outbox_unsafe_envelope";

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

pub(super) fn unsafe_envelope_error() -> AuditOutboxSinkError {
    AuditOutboxSinkError::Failed {
        safe_error: SAFE_ERROR_UNSAFE_ENVELOPE,
    }
}

pub fn sink_unavailable_error() -> AuditOutboxSinkError {
    AuditOutboxSinkError::Retryable {
        safe_error: SAFE_ERROR_SINK_UNAVAILABLE,
    }
}

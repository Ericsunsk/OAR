use std::error::Error;
use std::fmt;

use async_trait::async_trait;
use reqwest::Url;

use crate::oauth::{AsyncHttpClient, HttpRequest};

use super::{
    AuditOutboxDeliveryEnvelope, AuditOutboxSink, AuditOutboxSinkDelivery, AuditOutboxSinkError,
};

const DEFAULT_WEBHOOK_MAX_RESPONSE_BYTES: usize = 8 * 1024;

#[derive(Clone, PartialEq, Eq)]
pub enum WebhookAuditOutboxSinkConfigError {
    InvalidEndpoint,
    InvalidMaxResponseBytes,
}

impl fmt::Debug for WebhookAuditOutboxSinkConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebhookAuditOutboxSinkConfigError")
            .field("safe_error", &self.to_string())
            .finish()
    }
}

impl fmt::Display for WebhookAuditOutboxSinkConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEndpoint => write!(f, "audit_outbox_webhook_endpoint_invalid"),
            Self::InvalidMaxResponseBytes => {
                write!(f, "audit_outbox_webhook_max_response_bytes_invalid")
            }
        }
    }
}

impl Error for WebhookAuditOutboxSinkConfigError {}

pub struct WebhookAuditOutboxSink<H> {
    endpoint: String,
    http_client: H,
    max_response_bytes: usize,
}

impl<H> WebhookAuditOutboxSink<H> {
    pub fn new(
        endpoint: impl Into<String>,
        http_client: H,
    ) -> Result<Self, WebhookAuditOutboxSinkConfigError> {
        Self::with_max_response_bytes(endpoint, http_client, DEFAULT_WEBHOOK_MAX_RESPONSE_BYTES)
    }

    pub fn with_max_response_bytes(
        endpoint: impl Into<String>,
        http_client: H,
        max_response_bytes: usize,
    ) -> Result<Self, WebhookAuditOutboxSinkConfigError> {
        let endpoint = endpoint.into();
        validate_webhook_endpoint(&endpoint)?;
        if max_response_bytes == 0 {
            return Err(WebhookAuditOutboxSinkConfigError::InvalidMaxResponseBytes);
        }
        Ok(Self {
            endpoint,
            http_client,
            max_response_bytes,
        })
    }

    pub fn http_client(&self) -> &H {
        &self.http_client
    }
}

impl<H> fmt::Debug for WebhookAuditOutboxSink<H> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebhookAuditOutboxSink")
            .field("endpoint", &"[REDACTED]")
            .field("max_response_bytes", &self.max_response_bytes)
            .finish()
    }
}

#[async_trait]
impl<H> AuditOutboxSink for WebhookAuditOutboxSink<H>
where
    H: AsyncHttpClient + Send,
{
    async fn deliver(
        &mut self,
        envelope: AuditOutboxDeliveryEnvelope,
    ) -> Result<AuditOutboxSinkDelivery, AuditOutboxSinkError> {
        let request = webhook_request(&self.endpoint, self.max_response_bytes, envelope)?;
        let response = self
            .http_client
            .post_json(request)
            .await
            .map_err(|_| AuditOutboxSinkError::retryable())?;
        Ok(delivery_for_status(response.status))
    }
}

fn webhook_request(
    endpoint: &str,
    max_response_bytes: usize,
    envelope: AuditOutboxDeliveryEnvelope,
) -> Result<HttpRequest, AuditOutboxSinkError> {
    let delivery_id = envelope.delivery_id.clone();
    let tenant_id = envelope.tenant_id.clone();
    let body = serde_json::to_value(envelope).map_err(|_| AuditOutboxSinkError::failed())?;
    Ok(HttpRequest {
        method: "POST".to_string(),
        url: endpoint.to_string(),
        headers: vec![
            ("Idempotency-Key".to_string(), delivery_id.clone()),
            ("X-OAR-Delivery-ID".to_string(), delivery_id),
            ("X-OAR-Tenant-ID".to_string(), tenant_id),
        ],
        body,
        max_response_bytes,
    })
}

fn delivery_for_status(status: u16) -> AuditOutboxSinkDelivery {
    match status {
        200..=299 => AuditOutboxSinkDelivery::Sent,
        408 | 429 | 500..=599 => AuditOutboxSinkDelivery::Retryable,
        _ => AuditOutboxSinkDelivery::Failed,
    }
}

fn validate_webhook_endpoint(value: &str) -> Result<(), WebhookAuditOutboxSinkConfigError> {
    let endpoint =
        Url::parse(value).map_err(|_| WebhookAuditOutboxSinkConfigError::InvalidEndpoint)?;
    if endpoint.scheme() == "https" && endpoint.host().is_some() {
        Ok(())
    } else {
        Err(WebhookAuditOutboxSinkConfigError::InvalidEndpoint)
    }
}

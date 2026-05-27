use std::future::Future;

use super::{AuditOutboxMessage, PostgresAuditEventRepository, PostgresRepositoryError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditOutboxDrainConfig {
    pub tenant_id: String,
    pub stream: String,
    pub batch_limit: i64,
    pub lease_ms: u64,
    pub retry_delay_ms: u64,
    pub max_attempts: u32,
}

impl AuditOutboxDrainConfig {
    pub fn new(
        tenant_id: impl Into<String>,
        stream: impl Into<String>,
        batch_limit: i64,
        lease_ms: u64,
        retry_delay_ms: u64,
        max_attempts: u32,
    ) -> Self {
        Self {
            tenant_id: tenant_id.into(),
            stream: stream.into(),
            batch_limit,
            lease_ms,
            retry_delay_ms,
            max_attempts,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditOutboxDrainReport {
    pub claimed: usize,
    pub sent: usize,
    pub retryable: usize,
    pub failed: usize,
    pub exhausted: usize,
    pub stale: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditOutboxDelivery {
    Sent,
    Retryable,
    Failed,
}

pub trait AuditOutboxDispatcher {
    type Error;

    fn deliver(
        &mut self,
        message: &AuditOutboxMessage,
    ) -> impl Future<Output = Result<AuditOutboxDelivery, Self::Error>> + Send;
}

pub struct PostgresAuditOutboxWorker<D, C = fn() -> u64>
where
    D: AuditOutboxDispatcher,
    C: FnMut() -> u64,
{
    repository: PostgresAuditEventRepository,
    dispatcher: D,
    clock_ms: C,
    config: AuditOutboxDrainConfig,
}

impl<D, C> PostgresAuditOutboxWorker<D, C>
where
    D: AuditOutboxDispatcher,
    C: FnMut() -> u64,
{
    pub fn new(
        repository: PostgresAuditEventRepository,
        dispatcher: D,
        clock_ms: C,
        config: AuditOutboxDrainConfig,
    ) -> Self {
        Self {
            repository,
            dispatcher,
            clock_ms,
            config,
        }
    }

    pub async fn drain_once(&mut self) -> Result<AuditOutboxDrainReport, PostgresRepositoryError> {
        let now_ms = self.now_ms();
        let lease_until_ms = now_ms.saturating_add(self.config.lease_ms);
        let messages = self
            .repository
            .claim_outbox(
                &self.config.tenant_id,
                &self.config.stream,
                now_ms,
                self.config.batch_limit,
                lease_until_ms,
            )
            .await?;

        let mut report = AuditOutboxDrainReport {
            claimed: messages.len(),
            sent: 0,
            retryable: 0,
            failed: 0,
            exhausted: 0,
            stale: 0,
        };

        for message in messages {
            let delivery = self
                .dispatcher
                .deliver(&message)
                .await
                .unwrap_or(AuditOutboxDelivery::Retryable);
            let exhausted = matches!(delivery, AuditOutboxDelivery::Retryable)
                && u32::try_from(message.attempt_count)
                    .map(|attempt_count| attempt_count >= self.config.max_attempts)
                    .unwrap_or(false);
            let effective_delivery = if exhausted {
                AuditOutboxDelivery::Failed
            } else {
                delivery
            };
            let mark_result = match effective_delivery {
                AuditOutboxDelivery::Sent => {
                    let sent_at_ms = self.now_ms();
                    self.repository
                        .mark_outbox_sent_for_attempt(
                            &message.tenant_id,
                            message.id,
                            message.attempt_count,
                            lease_until_ms,
                            sent_at_ms,
                        )
                        .await?
                }
                AuditOutboxDelivery::Retryable => {
                    let next_attempt_at_ms =
                        self.now_ms().saturating_add(self.config.retry_delay_ms);
                    self.repository
                        .mark_outbox_retryable_for_attempt(
                            &message.tenant_id,
                            message.id,
                            message.attempt_count,
                            lease_until_ms,
                            next_attempt_at_ms,
                        )
                        .await?
                }
                AuditOutboxDelivery::Failed => {
                    self.repository
                        .mark_outbox_failed_for_attempt(
                            &message.tenant_id,
                            message.id,
                            message.attempt_count,
                            lease_until_ms,
                        )
                        .await?
                }
            };

            if !mark_result {
                report.stale += 1;
                continue;
            }

            match effective_delivery {
                AuditOutboxDelivery::Sent => report.sent += 1,
                AuditOutboxDelivery::Retryable => report.retryable += 1,
                AuditOutboxDelivery::Failed => {
                    report.failed += 1;
                    if exhausted {
                        report.exhausted += 1;
                    }
                }
            }
        }

        Ok(report)
    }

    pub fn dispatcher(&self) -> &D {
        &self.dispatcher
    }

    fn now_ms(&mut self) -> u64 {
        (self.clock_ms)()
    }
}

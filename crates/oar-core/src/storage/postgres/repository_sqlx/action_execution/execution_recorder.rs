use super::*;

impl PostgresExecutionRecorder {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn record_confirmation(
        &self,
        action: &ConfirmedAction,
        confirmed_at_ms: u64,
        operation_id: &str,
        event: &AuditEvent,
        outbox: &AuditOutboxEnvelope,
    ) -> PgRepositoryResult<PostgresExecutionRecorderReport> {
        super::validate_recorder_tenant(&action.tenant_id, event, outbox)?;

        let mut tx = self.pool.begin().await?;
        let submit =
            super::submit_confirmed_action_in_tx(&mut tx, action, confirmed_at_ms, operation_id)
                .await?;
        let (operation, duplicate) = super::submit_result_parts(submit);

        let outbox_id = if duplicate {
            None
        } else {
            super::audit::append_audit_event_in_tx(&mut tx, event, Some(&operation.operation_id))
                .await?;
            Some(super::audit::enqueue_outbox_in_tx(&mut tx, outbox).await?)
        };
        tx.commit().await?;

        Ok(PostgresExecutionRecorderReport {
            operation,
            outbox_id,
            inbox_item_id: None,
            duplicate,
        })
    }

    pub async fn record_dry_run(
        &self,
        tenant_id: &str,
        idempotency_key: &str,
        now_ms: u64,
        event: &AuditEvent,
        outbox: &AuditOutboxEnvelope,
    ) -> PgRepositoryResult<PostgresExecutionRecorderReport> {
        self.record_status_transition(StatusTransitionRequest {
            sql: MARK_EXECUTING,
            target_status: ActionStatus::Executing,
            tenant_id,
            idempotency_key,
            error: None,
            now_ms,
            event,
            outbox,
        })
        .await
    }

    pub async fn record_success(
        &self,
        tenant_id: &str,
        idempotency_key: &str,
        now_ms: u64,
        event: &AuditEvent,
        outbox: &AuditOutboxEnvelope,
    ) -> PgRepositoryResult<PostgresExecutionRecorderReport> {
        self.record_status_transition(StatusTransitionRequest {
            sql: MARK_SUCCEEDED,
            target_status: ActionStatus::Succeeded,
            tenant_id,
            idempotency_key,
            error: None,
            now_ms,
            event,
            outbox,
        })
        .await
    }

    pub async fn record_failure(
        &self,
        tenant_id: &str,
        idempotency_key: &str,
        error: &str,
        now_ms: u64,
        event: &AuditEvent,
        outbox: &AuditOutboxEnvelope,
    ) -> PgRepositoryResult<PostgresExecutionRecorderReport> {
        self.record_status_transition(StatusTransitionRequest {
            sql: MARK_FAILED,
            target_status: ActionStatus::Failed,
            tenant_id,
            idempotency_key,
            error: Some(error),
            now_ms,
            event,
            outbox,
        })
        .await
    }

    async fn record_status_transition(
        &self,
        request: StatusTransitionRequest<'_>,
    ) -> PgRepositoryResult<PostgresExecutionRecorderReport> {
        super::validate_recorder_tenant(request.tenant_id, request.event, request.outbox)?;

        let mut tx = self.pool.begin().await?;
        let (operation, duplicate) = super::transition_in_tx(
            &mut tx,
            request.sql,
            request.target_status,
            request.tenant_id,
            request.idempotency_key,
            request.error,
            request.now_ms,
        )
        .await?;

        let (inbox_item_id, outbox_id) = if duplicate {
            (None, None)
        } else {
            let inbox_item_id = super::review_inbox::update_review_inbox_ledger_projection_in_tx(
                &mut tx,
                &operation,
                request.target_status,
                request.now_ms,
            )
            .await?;
            super::audit::append_audit_event_in_tx(
                &mut tx,
                request.event,
                Some(&operation.operation_id),
            )
            .await?;
            (
                inbox_item_id,
                Some(super::audit::enqueue_outbox_in_tx(&mut tx, request.outbox).await?),
            )
        };
        tx.commit().await?;

        Ok(PostgresExecutionRecorderReport {
            operation,
            outbox_id,
            inbox_item_id,
            duplicate,
        })
    }
}

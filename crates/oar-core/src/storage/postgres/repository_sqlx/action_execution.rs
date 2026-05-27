use super::*;

impl PostgresReviewDecisionUnitOfWork {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn record_decision(
        &self,
        request: PostgresReviewDecisionUnitOfWorkRequest<'_>,
    ) -> PgRepositoryResult<PostgresReviewDecisionUnitOfWorkReport> {
        validate_review_decision_request(&request)?;

        let mut tx = self.pool.begin().await?;
        let inserted_decision = super::review_inbox::insert_proposed_action_decision_in_tx(
            &mut tx,
            request.decision.clone(),
        )
        .await?;

        if !inserted_decision {
            tx.commit().await?;
            return Ok(PostgresReviewDecisionUnitOfWorkReport {
                operation: None,
                inbox_item_id: None,
                outbox_id: None,
                duplicate: true,
            });
        }

        let operation = match (
            request.confirmed_action,
            request.confirmed_at_ms,
            request.operation_id,
        ) {
            (Some(action), Some(confirmed_at_ms), Some(operation_id)) => {
                let submit = super::action_execution::submit_confirmed_action_in_tx(
                    &mut tx,
                    action,
                    confirmed_at_ms,
                    operation_id,
                )
                .await?;
                let (operation, _) = submit_result_parts(submit);
                Some(operation)
            }
            _ => None,
        };

        let inbox_item_id =
            super::review_inbox::upsert_review_inbox_item_in_tx(&mut tx, request.inbox_item)
                .await?;
        super::audit::append_audit_event_in_tx(
            &mut tx,
            request.event,
            operation
                .as_ref()
                .map(|operation| operation.operation_id.as_str()),
        )
        .await?;
        let outbox_id = super::audit::enqueue_outbox_in_tx(&mut tx, request.outbox).await?;
        tx.commit().await?;

        Ok(PostgresReviewDecisionUnitOfWorkReport {
            operation,
            inbox_item_id,
            outbox_id: Some(outbox_id),
            duplicate: false,
        })
    }
}

impl PostgresOperationLedgerRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn submit_confirmed_action(
        &self,
        action: &ConfirmedAction,
        confirmed_at_ms: u64,
        operation_id: &str,
    ) -> PgRepositoryResult<SubmitResult> {
        if action.status != ActionStatus::Confirmed {
            return Err(PostgresRepositoryError::ActionNotConfirmed(action.status));
        }

        super::action_execution::submit_confirmed_action_with_executor(
            &self.pool,
            action,
            confirmed_at_ms,
            operation_id,
        )
        .await
    }

    pub async fn mark_executing(
        &self,
        tenant_id: &str,
        idempotency_key: &str,
        now_ms: u64,
    ) -> Result<OperationRecord, LedgerError> {
        self.transition(MARK_EXECUTING, tenant_id, idempotency_key, None, now_ms)
            .await
    }

    pub async fn mark_succeeded(
        &self,
        tenant_id: &str,
        idempotency_key: &str,
        now_ms: u64,
    ) -> Result<OperationRecord, LedgerError> {
        self.transition(MARK_SUCCEEDED, tenant_id, idempotency_key, None, now_ms)
            .await
    }

    pub async fn mark_failed(
        &self,
        tenant_id: &str,
        idempotency_key: &str,
        error: &str,
        now_ms: u64,
    ) -> Result<OperationRecord, LedgerError> {
        self.transition(MARK_FAILED, tenant_id, idempotency_key, Some(error), now_ms)
            .await
    }

    pub async fn get_by_idempotency_key(
        &self,
        tenant_id: &str,
        idempotency_key: &str,
    ) -> PgRepositoryResult<Option<OperationRecord>> {
        let row = sqlx::query(GET_BY_IDEMPOTENCY_KEY)
            .bind(tenant_id)
            .bind(idempotency_key)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(operation_record_from_row).transpose()
    }

    async fn transition(
        &self,
        sql: &'static str,
        tenant_id: &str,
        idempotency_key: &str,
        error: Option<&str>,
        now_ms: u64,
    ) -> Result<OperationRecord, LedgerError> {
        let target_status = match sql {
            MARK_EXECUTING => ActionStatus::Executing,
            MARK_SUCCEEDED => ActionStatus::Succeeded,
            MARK_FAILED => ActionStatus::Failed,
            _ => ActionStatus::Failed,
        };
        let safe_error = error.map(crate::action::safety::sanitize_adapter_error_message);
        let row = match safe_error.as_deref() {
            Some(error) => {
                sqlx::query(sql)
                    .bind(tenant_id)
                    .bind(idempotency_key)
                    .bind(error)
                    .bind(now_ms as i64)
                    .fetch_optional(&self.pool)
                    .await
            }
            None => {
                sqlx::query(sql)
                    .bind(tenant_id)
                    .bind(idempotency_key)
                    .bind(now_ms as i64)
                    .fetch_optional(&self.pool)
                    .await
            }
        }
        .map_err(|error| LedgerError::RepositoryFailure(error.to_string()))?;

        if let Some(row) = row {
            return operation_record_from_row(&row)
                .map_err(|error| LedgerError::RepositoryFailure(error.to_string()));
        }

        match self
            .get_by_idempotency_key(tenant_id, idempotency_key)
            .await
        {
            Ok(Some(record)) if record.status == target_status => Ok(record),
            Ok(Some(record)) => Err(LedgerError::InvalidTransition {
                from: record.status,
                to: target_status,
            }),
            Ok(None) => Err(LedgerError::UnknownIdempotencyKey(
                idempotency_key.to_string(),
            )),
            Err(error) => Err(LedgerError::RepositoryFailure(error.to_string())),
        }
    }
}

impl PostgresExecutionUnitOfWork {
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
    ) -> PgRepositoryResult<PostgresExecutionUnitOfWorkReport> {
        validate_uow_tenant(&action.tenant_id, event, outbox)?;

        let mut tx = self.pool.begin().await?;
        let submit = super::action_execution::submit_confirmed_action_in_tx(
            &mut tx,
            action,
            confirmed_at_ms,
            operation_id,
        )
        .await?;
        let (operation, duplicate) = submit_result_parts(submit);

        let outbox_id = if duplicate {
            None
        } else {
            super::audit::append_audit_event_in_tx(&mut tx, event, Some(&operation.operation_id))
                .await?;
            Some(super::audit::enqueue_outbox_in_tx(&mut tx, outbox).await?)
        };
        tx.commit().await?;

        Ok(PostgresExecutionUnitOfWorkReport {
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
    ) -> PgRepositoryResult<PostgresExecutionUnitOfWorkReport> {
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
    ) -> PgRepositoryResult<PostgresExecutionUnitOfWorkReport> {
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
    ) -> PgRepositoryResult<PostgresExecutionUnitOfWorkReport> {
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
    ) -> PgRepositoryResult<PostgresExecutionUnitOfWorkReport> {
        validate_uow_tenant(request.tenant_id, request.event, request.outbox)?;

        let mut tx = self.pool.begin().await?;
        let (operation, duplicate) = super::action_execution::transition_in_tx(
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

        Ok(PostgresExecutionUnitOfWorkReport {
            operation,
            outbox_id,
            inbox_item_id,
            duplicate,
        })
    }
}

pub(super) async fn submit_confirmed_action_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    action: &ConfirmedAction,
    confirmed_at_ms: u64,
    operation_id: &str,
) -> PgRepositoryResult<SubmitResult> {
    if action.status != ActionStatus::Confirmed {
        return Err(PostgresRepositoryError::ActionNotConfirmed(action.status));
    }

    submit_confirmed_action_with_executor(&mut **tx, action, confirmed_at_ms, operation_id).await
}

fn submit_result_parts(result: SubmitResult) -> (OperationRecord, bool) {
    match result {
        SubmitResult::Created(record) => (record, false),
        SubmitResult::Existing(record) => (record, true),
    }
}

pub(super) async fn submit_confirmed_action_with_executor<'e, E>(
    executor: E,
    action: &ConfirmedAction,
    confirmed_at_ms: u64,
    operation_id: &str,
) -> PgRepositoryResult<SubmitResult>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let row = sqlx::query(SUBMIT_CONFIRMED_ACTION_AND_LEDGER)
        .bind(&action.action_id)
        .bind(&action.tenant_id)
        .bind(&action.actor_user_id)
        .bind(&action.idempotency_key)
        .bind(confirmed_at_ms as i64)
        .bind(operation_id)
        .fetch_one(executor)
        .await?;
    let created: bool = row.try_get("created")?;
    let record = operation_record_from_row(&row)?;

    if created {
        Ok(SubmitResult::Created(record))
    } else {
        Ok(SubmitResult::Existing(record))
    }
}

fn validate_uow_tenant(
    expected_tenant_id: &str,
    event: &AuditEvent,
    outbox: &AuditOutboxEnvelope,
) -> PgRepositoryResult<()> {
    if event.scope.tenant_id != expected_tenant_id {
        return Err(PostgresRepositoryError::TenantMismatch {
            field: "event.scope.tenant_id",
            expected: expected_tenant_id.to_string(),
            actual: event.scope.tenant_id.clone(),
        });
    }

    if outbox.tenant_id != expected_tenant_id {
        return Err(PostgresRepositoryError::TenantMismatch {
            field: "outbox.tenant_id",
            expected: expected_tenant_id.to_string(),
            actual: outbox.tenant_id.clone(),
        });
    }

    Ok(())
}

fn validate_review_decision_tenant_binding(
    field: &'static str,
    expected: &str,
    actual: &str,
) -> PgRepositoryResult<()> {
    if expected != actual {
        return Err(PostgresRepositoryError::ReviewDecisionRequestMismatch {
            field,
            expected: expected.to_string(),
            actual: actual.to_string(),
        });
    }
    Ok(())
}

fn validate_review_decision_request(
    request: &PostgresReviewDecisionUnitOfWorkRequest<'_>,
) -> PgRepositoryResult<()> {
    validate_uow_tenant(request.decision.tenant_id, request.event, request.outbox)?;
    validate_review_decision_tenant_binding(
        "inbox_item.tenant_id",
        request.decision.tenant_id,
        &request.inbox_item.tenant_id.0,
    )?;
    validate_review_decision_tenant_binding(
        "event.actor.actor_id",
        request.decision.actor_user_id,
        &request.event.actor.actor_id,
    )?;
    validate_review_decision_tenant_binding(
        "inbox_item.user_id",
        request.decision.actor_user_id,
        &request.inbox_item.user_id.0,
    )?;
    validate_review_decision_tenant_binding(
        "inbox_item.proposed_action_id",
        request.decision.proposed_action_id,
        &request.inbox_item.proposed_action_id,
    )?;

    if request.decision.proposed_action_version != request.inbox_item.proposed_action_version {
        return Err(PostgresRepositoryError::ReviewDecisionRequestMismatch {
            field: "proposed_action_version",
            expected: request.decision.proposed_action_version.to_string(),
            actual: request.inbox_item.proposed_action_version.to_string(),
        });
    }

    let decision_requires_action = matches!(
        request.decision.decision,
        ProposedActionDecision::Confirm | ProposedActionDecision::EditThenConfirm { .. }
    );
    if decision_requires_action {
        let Some(action) = request.confirmed_action else {
            return Err(PostgresRepositoryError::MissingConfirmedActionForDecision);
        };
        let Some(operation_id) = request.operation_id else {
            return Err(PostgresRepositoryError::MissingOperationIdForDecision);
        };
        if request.confirmed_at_ms.is_none() {
            return Err(PostgresRepositoryError::MissingConfirmedAtForDecision);
        }

        let confirmed_action_id = request
            .decision
            .confirmed_action_id
            .ok_or(PostgresRepositoryError::MissingConfirmedActionForDecision)?;
        validate_review_decision_tenant_binding(
            "decision.confirmed_action_id",
            confirmed_action_id,
            &action.action_id,
        )?;
        validate_review_decision_tenant_binding(
            "confirmed_action.tenant_id",
            request.decision.tenant_id,
            &action.tenant_id,
        )?;
        validate_review_decision_tenant_binding(
            "confirmed_action.actor_user_id",
            request.decision.actor_user_id,
            &action.actor_user_id,
        )?;
        validate_review_decision_tenant_binding(
            "inbox_item.operation_id",
            operation_id,
            request
                .inbox_item
                .operation_id
                .as_deref()
                .unwrap_or_default(),
        )?;
        if request.inbox_item.ledger_status.is_none() {
            return Err(PostgresRepositoryError::ReviewDecisionRequestMismatch {
                field: "inbox_item.ledger_status",
                expected: "confirmed".to_string(),
                actual: "none".to_string(),
            });
        }
    } else {
        if request.confirmed_action.is_some() {
            return Err(PostgresRepositoryError::UnexpectedConfirmedActionForDecision);
        }
        if request.confirmed_at_ms.is_some() {
            return Err(PostgresRepositoryError::UnexpectedConfirmedActionForDecision);
        }
        if request.operation_id.is_some() {
            return Err(PostgresRepositoryError::UnexpectedOperationIdForDecision);
        }
        if request.decision.confirmed_action_id.is_some() {
            return Err(PostgresRepositoryError::UnexpectedConfirmedActionForDecision);
        }
    }

    Ok(())
}

pub(super) async fn transition_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    sql: &'static str,
    target_status: ActionStatus,
    tenant_id: &str,
    idempotency_key: &str,
    error: Option<&str>,
    now_ms: u64,
) -> PgRepositoryResult<(OperationRecord, bool)> {
    let safe_error = error.map(crate::action::safety::sanitize_adapter_error_message);
    let row = match safe_error.as_deref() {
        Some(error) => {
            sqlx::query(sql)
                .bind(tenant_id)
                .bind(idempotency_key)
                .bind(error)
                .bind(now_ms as i64)
                .fetch_optional(&mut **tx)
                .await?
        }
        None => {
            sqlx::query(sql)
                .bind(tenant_id)
                .bind(idempotency_key)
                .bind(now_ms as i64)
                .fetch_optional(&mut **tx)
                .await?
        }
    };

    if let Some(row) = row {
        return Ok((operation_record_from_row(&row)?, false));
    }

    let existing = sqlx::query(GET_BY_IDEMPOTENCY_KEY)
        .bind(tenant_id)
        .bind(idempotency_key)
        .fetch_optional(&mut **tx)
        .await?;

    match existing {
        Some(row) => {
            let record = operation_record_from_row(&row)?;
            if record.status == target_status {
                Ok((record, true))
            } else {
                Err(PostgresRepositoryError::InvalidOperationStatusTransition {
                    from: record.status,
                    to: target_status,
                })
            }
        }
        None => Err(PostgresRepositoryError::UnknownOperationIdempotencyKey(
            idempotency_key.to_string(),
        )),
    }
}

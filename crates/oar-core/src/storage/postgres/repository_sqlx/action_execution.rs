use super::*;

mod execution_recorder;
mod operation_ledger;
mod review_decision;

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

fn validate_recorder_tenant(
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

fn validate_review_decision_request(
    request: &PostgresReviewDecisionRecorderRequest<'_>,
) -> PgRepositoryResult<()> {
    validate_recorder_tenant(request.decision.tenant_id, request.event, request.outbox)?;
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

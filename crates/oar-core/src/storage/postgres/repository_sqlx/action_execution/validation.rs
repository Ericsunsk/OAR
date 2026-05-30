use super::*;

pub(super) fn validate_recorder_tenant(
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

pub(super) fn validate_review_decision_request(
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
    if request.inbox_item.sync_cursor <= request.expected_sync_cursor_value {
        return Err(PostgresRepositoryError::ReviewDecisionRequestMismatch {
            field: "inbox_item.sync_cursor",
            expected: format!(">{}", request.expected_sync_cursor_value),
            actual: request.inbox_item.sync_cursor.to_string(),
        });
    }

    let decision_requires_action = matches!(
        request.decision.decision,
        ProposedActionDecision::Confirm | ProposedActionDecision::EditThenConfirm { .. }
    );
    if decision_requires_action {
        if request.inbox_item.status != ReviewInboxItemStatus::Confirmed {
            return Err(PostgresRepositoryError::ReviewDecisionRequestMismatch {
                field: "inbox_item.status",
                expected: "confirmed".to_string(),
                actual: format!("{:?}", request.inbox_item.status),
            });
        }
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
        if request.inbox_item.status != ReviewInboxItemStatus::Rejected {
            return Err(PostgresRepositoryError::ReviewDecisionRequestMismatch {
                field: "inbox_item.status",
                expected: "rejected".to_string(),
                actual: format!("{:?}", request.inbox_item.status),
            });
        }
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

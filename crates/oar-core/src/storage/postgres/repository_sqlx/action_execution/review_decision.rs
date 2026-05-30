use super::*;

impl PostgresReviewDecisionRecorder {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn load_review_decision_context(
        &self,
        request: PostgresReviewDecisionContextRequest<'_>,
    ) -> PgRepositoryResult<Option<StoredReviewDecisionContext>> {
        super::review_inbox::load_review_decision_context_from_pool(&self.pool, request).await
    }

    pub async fn record_decision(
        &self,
        request: PostgresReviewDecisionRecorderRequest<'_>,
    ) -> PgRepositoryResult<PostgresReviewDecisionRecorderReport> {
        validate_review_decision_request(&request)?;

        let mut tx = self.pool.begin().await?;
        let inserted_decision = super::review_inbox::insert_proposed_action_decision_in_tx(
            &mut tx,
            request.decision.clone(),
        )
        .await?;

        if !inserted_decision {
            if !existing_decision_matches(&mut tx, &request).await? {
                return Err(PostgresRepositoryError::ReviewDecisionRequestMismatch {
                    field: "decision",
                    expected: "same_existing_decision".to_string(),
                    actual: "conflicting_existing_decision".to_string(),
                });
            }
            tx.commit().await?;
            return Ok(PostgresReviewDecisionRecorderReport {
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
                let submit = super::submit_confirmed_action_in_tx(
                    &mut tx,
                    action,
                    confirmed_at_ms,
                    operation_id,
                )
                .await?;
                let (operation, _) = super::submit_result_parts(submit);
                Some(operation)
            }
            _ => None,
        };

        let inbox_item_id = super::review_inbox::update_review_inbox_decision_state_in_tx(
            &mut tx,
            request.inbox_item,
            request.expected_sync_cursor_value,
        )
        .await?
        .ok_or_else(|| PostgresRepositoryError::ReviewDecisionRequestMismatch {
            field: "inbox_item.sync_cursor",
            expected: request.expected_sync_cursor_value.to_string(),
            actual: "stale_or_terminal".to_string(),
        })?;
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

        Ok(PostgresReviewDecisionRecorderReport {
            operation,
            inbox_item_id: Some(inbox_item_id),
            outbox_id: Some(outbox_id),
            duplicate: false,
        })
    }
}

async fn existing_decision_matches(
    tx: &mut Transaction<'_, Postgres>,
    request: &PostgresReviewDecisionRecorderRequest<'_>,
) -> PgRepositoryResult<bool> {
    let row = sqlx::query(LOAD_PROPOSED_ACTION_DECISION_FOR_RECORDER)
        .bind(request.decision.tenant_id)
        .bind(request.decision.proposed_action_id)
        .bind(request.decision.proposed_action_version as i64)
        .fetch_optional(&mut **tx)
        .await?;
    let Some(row) = row else {
        return Ok(false);
    };

    let (expected_decision, expected_edited_payload) =
        proposed_action_decision_to_db(request.decision.decision);
    let actor_user_id: String = row.try_get("actor_user_id")?;
    let decision: String = row.try_get("decision")?;
    let edited_payload: Option<Value> = row.try_get("edited_payload")?;
    let confirmed_action_id: Option<String> = row.try_get("confirmed_action_id")?;

    Ok(actor_user_id == request.decision.actor_user_id
        && decision == expected_decision
        && edited_payload.as_ref() == expected_edited_payload
        && confirmed_action_id.as_deref() == request.decision.confirmed_action_id)
}

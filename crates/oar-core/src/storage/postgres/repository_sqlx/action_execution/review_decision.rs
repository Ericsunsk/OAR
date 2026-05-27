use super::*;

impl PostgresReviewDecisionRecorder {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
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

        Ok(PostgresReviewDecisionRecorderReport {
            operation,
            inbox_item_id,
            outbox_id: Some(outbox_id),
            duplicate: false,
        })
    }
}

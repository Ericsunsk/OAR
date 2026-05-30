use super::*;

pub(in crate::storage::postgres::repository_sqlx) fn operation_record_from_row(
    row: &PgRow,
) -> PgRepositoryResult<OperationRecord> {
    let status: String = row.try_get("status")?;
    Ok(OperationRecord {
        operation_id: row.try_get("operation_id")?,
        tenant_id: row.try_get("tenant_id")?,
        action_id: row.try_get("action_id")?,
        idempotency_key: row.try_get("idempotency_key")?,
        status: action_status_from_db(&status)?,
        last_error: row.try_get("last_error")?,
    })
}

pub(in crate::storage::postgres::repository_sqlx) fn pending_confirmed_action_from_row(
    row: &PgRow,
) -> PgRepositoryResult<StoredPendingConfirmedAction> {
    let status: String = row.try_get("action_status")?;
    let confirmed_at_ms =
        optional_non_negative_i64_to_u64(row.try_get("confirmed_at_ms")?, "confirmed_at_ms")?;
    let proposed_action_kind: String = row.try_get("proposed_action_kind")?;
    let proposed_action_custom_kind: Option<String> = row.try_get("proposed_action_custom_kind")?;
    let proposed_action_decision: String = row.try_get("proposed_action_decision")?;
    let decision = proposed_action_decision_kind_from_db(&proposed_action_decision)?;
    let suggested_payload: Value = row.try_get("suggested_payload")?;
    let edited_payload: Option<Value> = row.try_get("edited_payload")?;
    let (decision, effective_payload) = match decision {
        StoredProposedActionDecisionKind::Confirm => {
            (ConfirmedExecutionDecision::Confirm, suggested_payload)
        }
        StoredProposedActionDecisionKind::EditThenConfirm => (
            ConfirmedExecutionDecision::EditThenConfirm,
            edited_payload.ok_or(PostgresRepositoryError::InvalidExecutionQueueRow {
                field: "edited_payload",
                reason: "edit_then_confirm requires edited payload",
            })?,
        ),
        StoredProposedActionDecisionKind::Reject => {
            return Err(PostgresRepositoryError::InvalidExecutionQueueRow {
                field: "decision",
                reason: "reject decisions are not executable",
            });
        }
    };
    let proposed_action_version = non_negative_i64_to_u64(
        row.try_get("proposed_action_version")?,
        "proposed_action_version",
    )?;

    let action = ConfirmedAction {
        action_id: row.try_get("action_id")?,
        tenant_id: row.try_get("tenant_id")?,
        actor_user_id: row.try_get("actor_user_id")?,
        idempotency_key: row.try_get("idempotency_key")?,
        status: action_status_from_db(&status)?,
        confirmed_at: confirmed_at_ms.map(ms_to_system_time),
    };
    Ok(StoredPendingConfirmedAction {
        request: ConfirmedExecutionRequest {
            confirmed_action: action,
            proposed_action_id: row.try_get("proposed_action_id")?,
            proposed_action_version,
            action_kind: proposed_action_kind_from_db(
                &proposed_action_kind,
                proposed_action_custom_kind,
            )?,
            target_user_id: row.try_get("target_user_id")?,
            owner_user_id: row.try_get("owner_user_id")?,
            evidence_ids: row.try_get("evidence_ids")?,
            effective_payload,
            decision,
        },
        operation: operation_record_from_row(row)?,
    })
}

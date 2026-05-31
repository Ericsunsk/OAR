use super::*;

mod execution_recorder;
mod operation_ledger;
mod review_decision;
mod transition;
mod validation;

use transition::{
    get_operation_by_idempotency_key_with_executor,
    list_confirmed_actions_ready_for_execution_with_executor, resolve_transition_miss,
    transition_operation_with_executor,
};
pub(super) use transition::{transition_in_tx, OperationStatusTransition};
use validation::{validate_recorder_tenant, validate_review_decision_request};

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

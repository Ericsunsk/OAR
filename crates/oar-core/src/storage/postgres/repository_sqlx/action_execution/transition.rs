use super::*;

pub(super) async fn transition_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    transition: OperationStatusTransition,
    tenant_id: &str,
    idempotency_key: &str,
    error: Option<&str>,
    now_ms: u64,
) -> PgRepositoryResult<(OperationRecord, bool)> {
    if let Some(record) = transition_operation_with_executor(
        &mut **tx,
        transition,
        tenant_id,
        idempotency_key,
        error,
        now_ms,
    )
    .await?
    {
        return Ok((record, false));
    }

    let existing =
        get_operation_by_idempotency_key_with_executor(&mut **tx, tenant_id, idempotency_key)
            .await?;

    resolve_transition_miss(existing, transition, idempotency_key)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct OperationStatusTransition {
    sql: &'static str,
    target_status: ActionStatus,
}

impl OperationStatusTransition {
    pub(super) const fn mark_executing() -> Self {
        Self {
            sql: MARK_EXECUTING,
            target_status: ActionStatus::Executing,
        }
    }

    pub(super) const fn mark_succeeded() -> Self {
        Self {
            sql: MARK_SUCCEEDED,
            target_status: ActionStatus::Succeeded,
        }
    }

    pub(super) const fn mark_failed() -> Self {
        Self {
            sql: MARK_FAILED,
            target_status: ActionStatus::Failed,
        }
    }

    pub(super) const fn target_status(self) -> ActionStatus {
        self.target_status
    }
}

pub(super) async fn transition_operation_with_executor<'e, E>(
    executor: E,
    transition: OperationStatusTransition,
    tenant_id: &str,
    idempotency_key: &str,
    error: Option<&str>,
    now_ms: u64,
) -> PgRepositoryResult<Option<OperationRecord>>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let safe_error = error.map(crate::action::safety::sanitize_adapter_error_message);
    let row = match safe_error.as_deref() {
        Some(error) => {
            sqlx::query(transition.sql)
                .bind(tenant_id)
                .bind(idempotency_key)
                .bind(error)
                .bind(now_ms as i64)
                .fetch_optional(executor)
                .await?
        }
        None => {
            sqlx::query(transition.sql)
                .bind(tenant_id)
                .bind(idempotency_key)
                .bind(now_ms as i64)
                .fetch_optional(executor)
                .await?
        }
    };
    row.as_ref().map(operation_record_from_row).transpose()
}

pub(super) async fn get_operation_by_idempotency_key_with_executor<'e, E>(
    executor: E,
    tenant_id: &str,
    idempotency_key: &str,
) -> PgRepositoryResult<Option<OperationRecord>>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let existing = sqlx::query(GET_BY_IDEMPOTENCY_KEY)
        .bind(tenant_id)
        .bind(idempotency_key)
        .fetch_optional(executor)
        .await?;
    existing.as_ref().map(operation_record_from_row).transpose()
}

pub(super) async fn list_confirmed_actions_ready_for_execution_with_executor<'e, E>(
    executor: E,
    tenant_id: &str,
    limit: u32,
) -> PgRepositoryResult<Vec<StoredPendingConfirmedAction>>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let rows = sqlx::query(LIST_CONFIRMED_ACTIONS_READY_FOR_EXECUTION)
        .bind(tenant_id)
        .bind(i64::from(limit))
        .fetch_all(executor)
        .await?;

    rows.iter().map(pending_confirmed_action_from_row).collect()
}

pub(super) fn resolve_transition_miss(
    existing: Option<OperationRecord>,
    transition: OperationStatusTransition,
    idempotency_key: &str,
) -> PgRepositoryResult<(OperationRecord, bool)> {
    match existing {
        Some(record) => {
            if record.status == transition.target_status {
                Ok((record, true))
            } else {
                Err(PostgresRepositoryError::InvalidOperationStatusTransition {
                    from: record.status,
                    to: transition.target_status,
                })
            }
        }
        None => Err(PostgresRepositoryError::UnknownOperationIdempotencyKey(
            idempotency_key.to_string(),
        )),
    }
}

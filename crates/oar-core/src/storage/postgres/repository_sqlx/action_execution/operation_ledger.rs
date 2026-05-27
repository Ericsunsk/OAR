use super::*;

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

        super::submit_confirmed_action_with_executor(
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

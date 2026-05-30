use super::*;
use crate::storage::postgres::postgres_repository_safe_error_reason;

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
        self.transition(
            super::OperationStatusTransition::mark_executing(),
            tenant_id,
            idempotency_key,
            None,
            now_ms,
        )
        .await
    }

    pub async fn mark_succeeded(
        &self,
        tenant_id: &str,
        idempotency_key: &str,
        now_ms: u64,
    ) -> Result<OperationRecord, LedgerError> {
        self.transition(
            super::OperationStatusTransition::mark_succeeded(),
            tenant_id,
            idempotency_key,
            None,
            now_ms,
        )
        .await
    }

    pub async fn mark_failed(
        &self,
        tenant_id: &str,
        idempotency_key: &str,
        error: &str,
        now_ms: u64,
    ) -> Result<OperationRecord, LedgerError> {
        self.transition(
            super::OperationStatusTransition::mark_failed(),
            tenant_id,
            idempotency_key,
            Some(error),
            now_ms,
        )
        .await
    }

    pub async fn get_by_idempotency_key(
        &self,
        tenant_id: &str,
        idempotency_key: &str,
    ) -> PgRepositoryResult<Option<OperationRecord>> {
        super::get_operation_by_idempotency_key_with_executor(
            &self.pool,
            tenant_id,
            idempotency_key,
        )
        .await
    }

    pub async fn list_confirmed_actions_ready_for_execution(
        &self,
        tenant_id: &str,
        limit: u32,
    ) -> PgRepositoryResult<Vec<StoredPendingConfirmedAction>> {
        super::list_confirmed_actions_ready_for_execution_with_executor(
            &self.pool, tenant_id, limit,
        )
        .await
    }

    async fn transition(
        &self,
        transition: super::OperationStatusTransition,
        tenant_id: &str,
        idempotency_key: &str,
        error: Option<&str>,
        now_ms: u64,
    ) -> Result<OperationRecord, LedgerError> {
        if let Some(record) = super::transition_operation_with_executor(
            &self.pool,
            transition,
            tenant_id,
            idempotency_key,
            error,
            now_ms,
        )
        .await
        .map_err(ledger_error_from_repository_error)?
        {
            return Ok(record);
        }

        let existing = super::get_operation_by_idempotency_key_with_executor(
            &self.pool,
            tenant_id,
            idempotency_key,
        )
        .await
        .map_err(ledger_error_from_repository_error)?;

        super::resolve_transition_miss(existing, transition, idempotency_key)
            .map(|(record, _)| record)
            .map_err(ledger_error_from_repository_error)
    }
}

fn ledger_repository_failure(error: &PostgresRepositoryError) -> LedgerError {
    LedgerError::RepositoryFailure(postgres_repository_safe_error_reason(error).to_string())
}

fn ledger_error_from_repository_error(error: PostgresRepositoryError) -> LedgerError {
    match error {
        PostgresRepositoryError::InvalidOperationStatusTransition { from, to } => {
            LedgerError::InvalidTransition { from, to }
        }
        PostgresRepositoryError::UnknownOperationIdempotencyKey(idempotency_key) => {
            LedgerError::UnknownIdempotencyKey(idempotency_key)
        }
        error => ledger_repository_failure(&error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ledger_repository_failure_redacts_raw_repository_text() {
        let error = PostgresRepositoryError::UnknownTenantStatus(
            "raw tenant status with password".to_string(),
        );

        let mapped = ledger_repository_failure(&error);

        assert_eq!(
            mapped,
            LedgerError::RepositoryFailure("unknown_tenant_status".to_string())
        );
    }

    #[test]
    fn sqlx_ledger_repository_failure_redacts_raw_sqlx_text() {
        let mapped = ledger_error_from_repository_error(PostgresRepositoryError::Sqlx(
            sqlx::Error::Protocol("database detail with password".to_string()),
        ));

        assert_eq!(
            mapped,
            LedgerError::RepositoryFailure("postgres_query_failed".to_string())
        );
    }
}

use std::sync::{Arc, Mutex};

use super::confirmed_action::ConfirmedAction;
use super::operation_ledger::{LedgerError, OperationLedger, OperationRecord, SubmitResult};

pub trait OperationLedgerRepository {
    fn submit_confirmed_action(
        &self,
        action: &ConfirmedAction,
    ) -> Result<SubmitResult, LedgerError>;

    fn mark_executing(&self, idempotency_key: &str) -> Result<OperationRecord, LedgerError>;

    fn mark_succeeded(&self, idempotency_key: &str) -> Result<OperationRecord, LedgerError>;

    fn mark_failed(
        &self,
        idempotency_key: &str,
        error: String,
    ) -> Result<OperationRecord, LedgerError>;

    fn get_by_idempotency_key(&self, idempotency_key: &str) -> Option<OperationRecord>;
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryOperationLedgerRepository {
    ledger: Arc<Mutex<OperationLedger>>,
}

impl InMemoryOperationLedgerRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn submit_confirmed_action(
        &self,
        action: &ConfirmedAction,
    ) -> Result<SubmitResult, LedgerError> {
        <Self as OperationLedgerRepository>::submit_confirmed_action(self, action)
    }

    pub fn mark_executing(&self, idempotency_key: &str) -> Result<OperationRecord, LedgerError> {
        <Self as OperationLedgerRepository>::mark_executing(self, idempotency_key)
    }

    pub fn mark_succeeded(&self, idempotency_key: &str) -> Result<OperationRecord, LedgerError> {
        <Self as OperationLedgerRepository>::mark_succeeded(self, idempotency_key)
    }

    pub fn mark_failed(
        &self,
        idempotency_key: &str,
        error: impl Into<String>,
    ) -> Result<OperationRecord, LedgerError> {
        <Self as OperationLedgerRepository>::mark_failed(self, idempotency_key, error.into())
    }

    pub fn get_by_idempotency_key(&self, idempotency_key: &str) -> Option<OperationRecord> {
        <Self as OperationLedgerRepository>::get_by_idempotency_key(self, idempotency_key)
    }

    fn with_ledger<T>(
        &self,
        op: impl FnOnce(&mut OperationLedger) -> Result<T, LedgerError>,
    ) -> Result<T, LedgerError> {
        let mut ledger = self.ledger.lock().map_err(|_| {
            tracing::warn!("operation ledger mutex poisoned");
            LedgerError::RepositoryFailure("operation ledger unavailable".to_string())
        })?;
        op(&mut ledger)
    }
}

impl OperationLedgerRepository for InMemoryOperationLedgerRepository {
    fn submit_confirmed_action(
        &self,
        action: &ConfirmedAction,
    ) -> Result<SubmitResult, LedgerError> {
        self.with_ledger(|ledger| ledger.submit_confirmed_action(action))
    }

    fn mark_executing(&self, idempotency_key: &str) -> Result<OperationRecord, LedgerError> {
        self.with_ledger(|ledger| ledger.mark_executing(idempotency_key))
    }

    fn mark_succeeded(&self, idempotency_key: &str) -> Result<OperationRecord, LedgerError> {
        self.with_ledger(|ledger| ledger.mark_succeeded(idempotency_key))
    }

    fn mark_failed(
        &self,
        idempotency_key: &str,
        error: String,
    ) -> Result<OperationRecord, LedgerError> {
        self.with_ledger(|ledger| ledger.mark_failed(idempotency_key, error))
    }

    fn get_by_idempotency_key(&self, idempotency_key: &str) -> Option<OperationRecord> {
        match self.ledger.lock() {
            Ok(ledger) => ledger.get_by_idempotency_key(idempotency_key).cloned(),
            Err(_) => {
                tracing::warn!("operation ledger mutex poisoned while reading");
                None
            }
        }
    }
}

use std::collections::HashMap;

use super::confirmed_action::{ActionStatus, ConfirmedAction};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationRecord {
    pub operation_id: String,
    pub tenant_id: String,
    pub action_id: String,
    pub idempotency_key: String,
    pub status: ActionStatus,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmitResult {
    Created(OperationRecord),
    Existing(OperationRecord),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedgerError {
    ActionNotConfirmed {
        status: ActionStatus,
    },
    UnknownIdempotencyKey(String),
    RepositoryFailure(String),
    InvalidTransition {
        from: ActionStatus,
        to: ActionStatus,
    },
}

#[derive(Debug)]
pub struct OperationLedger {
    records_by_operation_id: HashMap<String, OperationRecord>,
    operation_id_by_idempotency_key: HashMap<String, String>,
    sequence: u64,
}

impl OperationLedger {
    pub fn new() -> Self {
        Self {
            records_by_operation_id: HashMap::new(),
            operation_id_by_idempotency_key: HashMap::new(),
            sequence: 0,
        }
    }

    pub fn submit_confirmed_action(
        &mut self,
        action: &ConfirmedAction,
    ) -> Result<SubmitResult, LedgerError> {
        if action.status != ActionStatus::Confirmed {
            return Err(LedgerError::ActionNotConfirmed {
                status: action.status,
            });
        }

        if let Some(existing) = self.record_by_idempotency_key(&action.idempotency_key) {
            return Ok(SubmitResult::Existing(existing.clone()));
        }

        self.sequence += 1;
        let operation_id = format!("op-{}", self.sequence);
        let record = OperationRecord {
            operation_id: operation_id.clone(),
            tenant_id: action.tenant_id.clone(),
            action_id: action.action_id.clone(),
            idempotency_key: action.idempotency_key.clone(),
            status: ActionStatus::Confirmed,
            last_error: None,
        };

        self.operation_id_by_idempotency_key
            .insert(action.idempotency_key.clone(), operation_id.clone());
        self.records_by_operation_id
            .insert(operation_id, record.clone());

        Ok(SubmitResult::Created(record))
    }

    pub fn mark_executing(
        &mut self,
        idempotency_key: &str,
    ) -> Result<OperationRecord, LedgerError> {
        self.transition(idempotency_key, ActionStatus::Executing)
    }

    pub fn mark_succeeded(
        &mut self,
        idempotency_key: &str,
    ) -> Result<OperationRecord, LedgerError> {
        self.transition(idempotency_key, ActionStatus::Succeeded)
    }

    pub fn mark_failed(
        &mut self,
        idempotency_key: &str,
        error: impl Into<String>,
    ) -> Result<OperationRecord, LedgerError> {
        let operation_id = self
            .operation_id_by_idempotency_key
            .get(idempotency_key)
            .cloned()
            .ok_or_else(|| LedgerError::UnknownIdempotencyKey(idempotency_key.to_string()))?;

        let mut record = self
            .records_by_operation_id
            .get(&operation_id)
            .cloned()
            .ok_or_else(|| LedgerError::UnknownIdempotencyKey(idempotency_key.to_string()))?;

        if record.status == ActionStatus::Failed {
            return Ok(record);
        }

        if record.status != ActionStatus::Executing {
            return Err(LedgerError::InvalidTransition {
                from: record.status,
                to: ActionStatus::Failed,
            });
        }

        record.status = ActionStatus::Failed;
        record.last_error = Some(error.into());
        self.records_by_operation_id
            .insert(record.operation_id.clone(), record.clone());
        Ok(record)
    }

    pub fn get_by_idempotency_key(&self, idempotency_key: &str) -> Option<&OperationRecord> {
        self.record_by_idempotency_key(idempotency_key)
    }

    fn transition(
        &mut self,
        idempotency_key: &str,
        to: ActionStatus,
    ) -> Result<OperationRecord, LedgerError> {
        let operation_id = self
            .operation_id_by_idempotency_key
            .get(idempotency_key)
            .cloned()
            .ok_or_else(|| LedgerError::UnknownIdempotencyKey(idempotency_key.to_string()))?;

        let mut record = self
            .records_by_operation_id
            .get(&operation_id)
            .cloned()
            .ok_or_else(|| LedgerError::UnknownIdempotencyKey(idempotency_key.to_string()))?;

        if record.status == to {
            return Ok(record);
        }

        let allowed = matches!(
            (record.status, to),
            (ActionStatus::Confirmed, ActionStatus::Executing)
                | (ActionStatus::Executing, ActionStatus::Succeeded)
                | (ActionStatus::Executing, ActionStatus::Failed)
        );

        if !allowed {
            return Err(LedgerError::InvalidTransition {
                from: record.status,
                to,
            });
        }

        record.status = to;
        if to != ActionStatus::Failed {
            record.last_error = None;
        }
        self.records_by_operation_id
            .insert(operation_id, record.clone());
        Ok(record)
    }

    fn record_by_idempotency_key(&self, idempotency_key: &str) -> Option<&OperationRecord> {
        let operation_id = self.operation_id_by_idempotency_key.get(idempotency_key)?;
        self.records_by_operation_id.get(operation_id)
    }
}

impl Default for OperationLedger {
    fn default() -> Self {
        Self::new()
    }
}

use std::fmt;

use crate::action::audit_event::AuditEvent;
use crate::action::audit_repository::AuditRepositoryError;
use crate::action::execution_policy::ExecutionDenied;
use crate::action::operation_ledger::{LedgerError, OperationRecord};

use super::adapter::AdapterError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionReport {
    pub operation: OperationRecord,
    pub events: Vec<AuditEvent>,
    pub duplicate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyDenialReport {
    pub denial: ExecutionDenied,
    pub events: Vec<AuditEvent>,
}

#[derive(Clone, PartialEq, Eq)]
pub enum ExecutionError {
    Ledger(LedgerError),
    Adapter(AdapterError),
    Audit(AuditRepositoryError),
    PolicyDenied(PolicyDenialReport),
}

impl fmt::Debug for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ledger(error) => f.debug_tuple("Ledger").field(error).finish(),
            Self::Adapter(error) => f.debug_tuple("Adapter").field(error).finish(),
            Self::Audit(error) => f.debug_tuple("Audit").field(error).finish(),
            Self::PolicyDenied(report) => f.debug_tuple("PolicyDenied").field(report).finish(),
        }
    }
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ledger(error) => write!(f, "ledger error: {error:?}"),
            Self::Adapter(error) => write!(f, "adapter error: {}", error.code),
            Self::Audit(error) => write!(f, "audit error: {error:?}"),
            Self::PolicyDenied(report) => write!(
                f,
                "execution denied by policy: {}",
                report
                    .events
                    .first()
                    .and_then(|event| event.execution.as_ref())
                    .and_then(|execution| execution.message.as_deref())
                    .unwrap_or("policy denied")
            ),
        }
    }
}

impl std::error::Error for ExecutionError {}

impl From<LedgerError> for ExecutionError {
    fn from(value: LedgerError) -> Self {
        Self::Ledger(value)
    }
}

impl From<AdapterError> for ExecutionError {
    fn from(value: AdapterError) -> Self {
        Self::Adapter(value)
    }
}

impl From<AuditRepositoryError> for ExecutionError {
    fn from(value: AuditRepositoryError) -> Self {
        Self::Audit(value)
    }
}

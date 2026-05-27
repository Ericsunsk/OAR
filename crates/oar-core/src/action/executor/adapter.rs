use std::fmt;

use crate::action::audit_event::AuditStateSummary;
use crate::action::safety::{sanitize_adapter_error_code, sanitize_adapter_error_message};

#[derive(Clone, PartialEq, Eq)]
pub struct AdapterError {
    pub code: String,
    pub safe_message: String,
}

impl AdapterError {
    pub fn from_safe_message(code: impl Into<String>, safe_message: impl Into<String>) -> Self {
        Self {
            code: sanitize_adapter_error_code(&code.into()),
            safe_message: sanitize_adapter_error_message(&safe_message.into()),
        }
    }
}

impl fmt::Debug for AdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AdapterError")
            .field("code", &self.code)
            .field("safe_message", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterDryRun {
    pub before: Option<AuditStateSummary>,
    pub after: Option<AuditStateSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterExecution {
    pub adapter_operation_id: String,
    pub before: Option<AuditStateSummary>,
    pub after: Option<AuditStateSummary>,
}

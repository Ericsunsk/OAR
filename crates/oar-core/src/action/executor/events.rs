use crate::action::audit_event::{AuditEvent, AuditStateSummary};
use crate::action::audit_trace::AuditTrace;
use crate::action::confirmed_action::ConfirmedAction;
use crate::action::execution_policy::ExecutionDenied;

use super::policy::safe_denial_message;

pub(crate) fn confirmed_action(
    occurred_at_ms: u64,
    trace: &mut AuditTrace,
    action: &ConfirmedAction,
) -> AuditEvent {
    trace.confirmed_action(
        occurred_at_ms,
        AuditStateSummary {
            summary: format!("confirmed action {}", action.action_id),
            reference_ids: vec![action.idempotency_key.clone()],
            content_hash: None,
        },
    )
}

pub(crate) fn dry_run(
    occurred_at_ms: u64,
    trace: &mut AuditTrace,
    before: Option<AuditStateSummary>,
    after: Option<AuditStateSummary>,
) -> AuditEvent {
    trace.dry_run(occurred_at_ms, before, after)
}

pub(crate) fn execution_succeeded(
    occurred_at_ms: u64,
    trace: &mut AuditTrace,
    before: Option<AuditStateSummary>,
    after: Option<AuditStateSummary>,
    adapter_operation_id: String,
) -> AuditEvent {
    trace.execution_succeeded(occurred_at_ms, before, after, adapter_operation_id)
}

pub(crate) fn execution_failed(
    occurred_at_ms: u64,
    trace: &mut AuditTrace,
    error_code: String,
    message: String,
) -> AuditEvent {
    trace.execution_failed(occurred_at_ms, None, None, error_code, message)
}

pub(crate) fn execution_denied(
    occurred_at_ms: u64,
    trace: &mut AuditTrace,
    denial: &ExecutionDenied,
) -> AuditEvent {
    trace.execution_denied(occurred_at_ms, "policy_denied", safe_denial_message(denial))
}

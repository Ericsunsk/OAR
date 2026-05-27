use serde::{Deserialize, Serialize};

/// Append-only audit event for safety and compliance traceability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEvent {
    pub event_id: String,
    pub trace_id: String,
    pub sequence: u64,
    pub occurred_at_ms: u64,
    pub event_type: AuditEventType,
    pub actor: AuditActor,
    pub scope: AuditScope,
    pub target: AuditTarget,
    pub before: Option<AuditStateSummary>,
    pub after: Option<AuditStateSummary>,
    pub execution: Option<ExecutionResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditEventType {
    ProposedActionDecisionRecorded,
    ConfirmedActionRecorded,
    DryRunExecuted,
    ExecutionDenied,
    ExecutionSucceeded,
    ExecutionFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditActorKind {
    User,
    Bot,
    App,
    System,
    Service,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditActor {
    pub kind: AuditActorKind,
    pub actor_id: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditScope {
    pub tenant_id: String,
    pub workspace_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditTarget {
    pub resource_type: String,
    pub resource_id: String,
    pub action_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditSubject {
    pub actor: AuditActor,
    pub scope: AuditScope,
    pub target: AuditTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEventContext {
    pub event_id: String,
    pub trace_id: String,
    pub sequence: u64,
    pub occurred_at_ms: u64,
    pub subject: AuditSubject,
}

/// Structured, non-sensitive summary only. Do not store raw tokens/secrets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditStateSummary {
    pub summary: String,
    pub reference_ids: Vec<String>,
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Succeeded,
    Failed,
    DryRun,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub status: ExecutionStatus,
    pub adapter_operation_id: Option<String>,
    pub error_code: Option<String>,
    pub message: Option<String>,
}

impl AuditEvent {
    pub fn proposed_action_decision(context: AuditEventContext, after: AuditStateSummary) -> Self {
        Self {
            event_id: context.event_id,
            trace_id: context.trace_id,
            sequence: context.sequence,
            occurred_at_ms: context.occurred_at_ms,
            event_type: AuditEventType::ProposedActionDecisionRecorded,
            actor: context.subject.actor,
            scope: context.subject.scope,
            target: context.subject.target,
            before: None,
            after: Some(after),
            execution: None,
        }
    }

    pub fn confirmed_action(context: AuditEventContext, after: AuditStateSummary) -> Self {
        Self {
            event_id: context.event_id,
            trace_id: context.trace_id,
            sequence: context.sequence,
            occurred_at_ms: context.occurred_at_ms,
            event_type: AuditEventType::ConfirmedActionRecorded,
            actor: context.subject.actor,
            scope: context.subject.scope,
            target: context.subject.target,
            before: None,
            after: Some(after),
            execution: None,
        }
    }

    pub fn dry_run(
        context: AuditEventContext,
        before: Option<AuditStateSummary>,
        after: Option<AuditStateSummary>,
    ) -> Self {
        Self {
            event_id: context.event_id,
            trace_id: context.trace_id,
            sequence: context.sequence,
            occurred_at_ms: context.occurred_at_ms,
            event_type: AuditEventType::DryRunExecuted,
            actor: context.subject.actor,
            scope: context.subject.scope,
            target: context.subject.target,
            before,
            after,
            execution: Some(ExecutionResult {
                status: ExecutionStatus::DryRun,
                adapter_operation_id: None,
                error_code: None,
                message: None,
            }),
        }
    }

    pub fn execution_succeeded(
        context: AuditEventContext,
        before: Option<AuditStateSummary>,
        after: Option<AuditStateSummary>,
        adapter_operation_id: impl Into<String>,
    ) -> Self {
        Self {
            event_id: context.event_id,
            trace_id: context.trace_id,
            sequence: context.sequence,
            occurred_at_ms: context.occurred_at_ms,
            event_type: AuditEventType::ExecutionSucceeded,
            actor: context.subject.actor,
            scope: context.subject.scope,
            target: context.subject.target,
            before,
            after,
            execution: Some(ExecutionResult {
                status: ExecutionStatus::Succeeded,
                adapter_operation_id: Some(adapter_operation_id.into()),
                error_code: None,
                message: None,
            }),
        }
    }

    pub fn execution_denied(
        context: AuditEventContext,
        error_code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            event_id: context.event_id,
            trace_id: context.trace_id,
            sequence: context.sequence,
            occurred_at_ms: context.occurred_at_ms,
            event_type: AuditEventType::ExecutionDenied,
            actor: context.subject.actor,
            scope: context.subject.scope,
            target: context.subject.target,
            before: None,
            after: None,
            execution: Some(ExecutionResult {
                status: ExecutionStatus::Denied,
                adapter_operation_id: None,
                error_code: Some(error_code.into()),
                message: Some(message.into()),
            }),
        }
    }

    pub fn execution_failed(
        context: AuditEventContext,
        before: Option<AuditStateSummary>,
        after: Option<AuditStateSummary>,
        error_code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            event_id: context.event_id,
            trace_id: context.trace_id,
            sequence: context.sequence,
            occurred_at_ms: context.occurred_at_ms,
            event_type: AuditEventType::ExecutionFailed,
            actor: context.subject.actor,
            scope: context.subject.scope,
            target: context.subject.target,
            before,
            after,
            execution: Some(ExecutionResult {
                status: ExecutionStatus::Failed,
                adapter_operation_id: None,
                error_code: Some(error_code.into()),
                message: Some(message.into()),
            }),
        }
    }
}

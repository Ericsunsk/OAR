use std::time::{SystemTime, UNIX_EPOCH};

use super::audit_event::{
    AuditActor, AuditActorKind, AuditEvent, AuditScope, AuditStateSummary, AuditSubject,
    AuditTarget,
};
use super::audit_repository::{
    AuditEventRepository, AuditRepositoryError, InMemoryAuditEventRepository,
};
use super::audit_trace::AuditTrace;
use super::confirmed_action::ConfirmedAction;
use super::execution_policy::{ActionActorBinding, ExecutionDenied, ExecutionPolicy};
use super::operation_ledger::{LedgerError, OperationRecord, SubmitResult};
use super::operation_ledger_repository::{
    InMemoryOperationLedgerRepository, OperationLedgerRepository,
};
use super::safety::{sanitize_adapter_error_code, sanitize_adapter_error_message};
use crate::domain::identity::TokenGrant;

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

#[derive(Debug, Clone, PartialEq, Eq)]
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

pub trait ActionAdapter {
    fn dry_run(&mut self, action: &ConfirmedAction) -> Result<AdapterDryRun, AdapterError>;
    fn execute(&mut self, action: &ConfirmedAction) -> Result<AdapterExecution, AdapterError>;
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionError {
    Ledger(LedgerError),
    Adapter(AdapterError),
    Audit(AuditRepositoryError),
    PolicyDenied(PolicyDenialReport),
}

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

pub struct ActionExecutor<
    A,
    C = fn() -> u64,
    L = InMemoryOperationLedgerRepository,
    R = InMemoryAuditEventRepository,
> where
    A: ActionAdapter,
    C: FnMut() -> u64,
    L: OperationLedgerRepository,
    R: AuditEventRepository,
{
    ledger: L,
    audit: R,
    adapter: A,
    clock_ms: C,
}

impl<A>
    ActionExecutor<A, fn() -> u64, InMemoryOperationLedgerRepository, InMemoryAuditEventRepository>
where
    A: ActionAdapter,
{
    pub fn new(adapter: A) -> Self {
        Self::with_clock(adapter, now_ms)
    }
}

impl<A, C> ActionExecutor<A, C, InMemoryOperationLedgerRepository, InMemoryAuditEventRepository>
where
    A: ActionAdapter,
    C: FnMut() -> u64,
{
    pub fn with_clock(adapter: A, clock_ms: C) -> Self {
        Self::with_repositories(
            adapter,
            clock_ms,
            InMemoryOperationLedgerRepository::new(),
            InMemoryAuditEventRepository::new(),
        )
    }
}

impl<A, C, L, R> ActionExecutor<A, C, L, R>
where
    A: ActionAdapter,
    C: FnMut() -> u64,
    L: OperationLedgerRepository,
    R: AuditEventRepository,
{
    pub fn with_repositories(adapter: A, clock_ms: C, ledger: L, audit: R) -> Self {
        Self {
            ledger,
            audit,
            adapter,
            clock_ms,
        }
    }

    pub fn execute_confirmed_action(
        &mut self,
        action: &ConfirmedAction,
    ) -> Result<ExecutionReport, ExecutionError> {
        match self.ledger.submit_confirmed_action(action)? {
            SubmitResult::Existing(existing) => Ok(ExecutionReport {
                operation: existing,
                events: Vec::new(),
                duplicate: true,
            }),
            SubmitResult::Created(created) => self.run_new_operation(action, created),
        }
    }

    pub fn execute_confirmed_action_with_policy(
        &mut self,
        action: &ConfirmedAction,
        action_type: &str,
        required_scope: &str,
        actor_binding: &ActionActorBinding,
        grant: &TokenGrant,
        policy: &ExecutionPolicy,
    ) -> Result<ExecutionReport, ExecutionError> {
        if let Err(denial) =
            policy.evaluate(action, action_type, required_scope, grant, actor_binding)
        {
            let mut trace = action_audit_trace(action);
            let event = self.event_denied(&mut trace, &denial);
            self.audit.append(event.clone())?;
            return Err(ExecutionError::PolicyDenied(PolicyDenialReport {
                denial,
                events: vec![event],
            }));
        }

        self.execute_confirmed_action(action)
    }

    pub fn ledger(&self) -> &L {
        &self.ledger
    }

    pub fn adapter(&self) -> &A {
        &self.adapter
    }

    pub fn audit(&self) -> &R {
        &self.audit
    }

    fn run_new_operation(
        &mut self,
        action: &ConfirmedAction,
        created: OperationRecord,
    ) -> Result<ExecutionReport, ExecutionError> {
        let mut trace = action_audit_trace(action);
        let mut events = Vec::new();
        let confirmed_event = self.event_confirmed(&mut trace, action);
        self.record_event(&mut events, confirmed_event)?;

        let dry_run = self.adapter.dry_run(action)?;
        let dry_run_event = self.event_dry_run(&mut trace, dry_run.before, dry_run.after);
        self.record_event(&mut events, dry_run_event)?;

        self.ledger.mark_executing(&action.idempotency_key)?;
        let execute_result = self.adapter.execute(action);

        let final_record = match execute_result {
            Ok(execution) => {
                let record = self.ledger.mark_succeeded(&action.idempotency_key)?;
                let succeeded_event = self.event_succeeded(
                    &mut trace,
                    execution.before,
                    execution.after,
                    execution.adapter_operation_id,
                );
                self.record_event(&mut events, succeeded_event)?;
                record
            }
            Err(error) => {
                let record = self
                    .ledger
                    .mark_failed(&action.idempotency_key, error.safe_message.clone())?;
                let failed_event =
                    self.event_failed(&mut trace, error.code.clone(), error.safe_message.clone());
                self.record_event(&mut events, failed_event)?;
                return Ok(ExecutionReport {
                    operation: record,
                    events,
                    duplicate: false,
                });
            }
        };

        Ok(ExecutionReport {
            operation: if final_record.operation_id == created.operation_id {
                final_record
            } else {
                created
            },
            events,
            duplicate: false,
        })
    }

    fn record_event(
        &self,
        events: &mut Vec<AuditEvent>,
        event: AuditEvent,
    ) -> Result<(), ExecutionError> {
        self.audit.append(event.clone())?;
        events.push(event);
        Ok(())
    }

    fn event_confirmed(&mut self, trace: &mut AuditTrace, action: &ConfirmedAction) -> AuditEvent {
        let occurred_at_ms = self.now_ms();
        trace.confirmed_action(
            occurred_at_ms,
            AuditStateSummary {
                summary: format!("confirmed action {}", action.action_id),
                reference_ids: vec![action.idempotency_key.clone()],
                content_hash: None,
            },
        )
    }

    fn event_dry_run(
        &mut self,
        trace: &mut AuditTrace,
        before: Option<AuditStateSummary>,
        after: Option<AuditStateSummary>,
    ) -> AuditEvent {
        let occurred_at_ms = self.now_ms();
        trace.dry_run(occurred_at_ms, before, after)
    }

    fn event_succeeded(
        &mut self,
        trace: &mut AuditTrace,
        before: Option<AuditStateSummary>,
        after: Option<AuditStateSummary>,
        adapter_operation_id: String,
    ) -> AuditEvent {
        let occurred_at_ms = self.now_ms();
        trace.execution_succeeded(occurred_at_ms, before, after, adapter_operation_id)
    }

    fn event_failed(
        &mut self,
        trace: &mut AuditTrace,
        error_code: String,
        message: String,
    ) -> AuditEvent {
        let occurred_at_ms = self.now_ms();
        trace.execution_failed(occurred_at_ms, None, None, error_code, message)
    }

    fn event_denied(&mut self, trace: &mut AuditTrace, denial: &ExecutionDenied) -> AuditEvent {
        let occurred_at_ms = self.now_ms();
        trace.execution_denied(occurred_at_ms, "policy_denied", safe_denial_message(denial))
    }

    fn now_ms(&mut self) -> u64 {
        (self.clock_ms)()
    }
}

pub(crate) fn action_audit_trace(action: &ConfirmedAction) -> AuditTrace {
    AuditTrace::new(action_trace_id(action), action_audit_subject(action))
}

pub(crate) fn action_trace_id(action: &ConfirmedAction) -> String {
    format!("trace-{}", action.idempotency_key)
}

pub(crate) fn action_audit_subject(action: &ConfirmedAction) -> AuditSubject {
    AuditSubject {
        actor: AuditActor {
            kind: AuditActorKind::User,
            actor_id: action.actor_user_id.clone(),
            display_name: None,
        },
        scope: AuditScope {
            tenant_id: action.tenant_id.clone(),
            workspace_id: None,
        },
        target: AuditTarget {
            resource_type: "confirmed_action".to_string(),
            resource_id: action.action_id.clone(),
            action_type: "execute".to_string(),
        },
    }
}

pub(crate) fn safe_denial_message(denial: &ExecutionDenied) -> String {
    match denial {
        ExecutionDenied::TenantMismatch { .. } => {
            "Execution denied by policy: action and token grant belong to different tenants"
                .to_string()
        }
        ExecutionDenied::ActionNotConfirmed { status } => {
            format!("Execution denied by policy: action status is {status:?}, not Confirmed")
        }
        ExecutionDenied::ActorUserMismatch { .. } => {
            "Execution denied by policy: action actor does not match bound actor".to_string()
        }
        ExecutionDenied::IdentityMismatch { .. } => {
            "Execution denied by policy: action actor is not authorized for token grant identity"
                .to_string()
        }
        ExecutionDenied::ActionNotAllowlisted { action_type } => {
            format!("Execution denied by policy: action type {action_type} is not allowlisted")
        }
        ExecutionDenied::ActorKindNotAllowed { actor_kind } => {
            format!("Execution denied by policy: actor kind {actor_kind:?} is not allowed")
        }
        ExecutionDenied::GrantNotExecutable { state } => {
            format!("Execution denied by policy: token grant state {state:?} is not executable")
        }
        ExecutionDenied::MissingScope { required_scope } => {
            format!("Execution denied by policy: missing required scope {required_scope}")
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::audit_event::AuditEventType;
    use crate::action::confirmed_action::ConfirmedAction;
    use std::time::SystemTime;

    #[derive(Debug)]
    struct FailingAdapter {
        code: String,
        message: String,
    }

    impl ActionAdapter for FailingAdapter {
        fn dry_run(&mut self, _action: &ConfirmedAction) -> Result<AdapterDryRun, AdapterError> {
            Ok(AdapterDryRun {
                before: None,
                after: None,
            })
        }

        fn execute(&mut self, _action: &ConfirmedAction) -> Result<AdapterExecution, AdapterError> {
            Err(AdapterError::from_safe_message(
                self.code.clone(),
                self.message.clone(),
            ))
        }
    }

    #[test]
    fn adapter_error_sanitizes_token_like_message() {
        let error = AdapterError::from_safe_message(
            "  raw-code ",
            "Authorization: Bearer tok_live_fake refresh_token=rt_live_fake",
        );
        assert_eq!(error.code, "raw-code");
        assert_eq!(error.safe_message, "adapter execution failed");
    }

    #[test]
    fn adapter_error_preserves_non_sensitive_message() {
        let error = AdapterError::from_safe_message(
            "adapter.timeout",
            "adapter timeout while calling lark",
        );
        assert_eq!(error.code, "adapter.timeout");
        assert_eq!(error.safe_message, "adapter timeout while calling lark");
    }

    #[test]
    fn execution_failure_records_safe_message_only() {
        let action = ConfirmedAction::proposed("act-1", "tenant-1", "user-1", "idem-1")
            .confirm(SystemTime::UNIX_EPOCH);
        let adapter = FailingAdapter {
            code: "execution failed".to_string(),
            message: "stderr: Authorization: Bearer at_secret_value".to_string(),
        };
        let mut executor = ActionExecutor::with_clock(adapter, || 1_u64);

        let report = executor
            .execute_confirmed_action(&action)
            .expect("execution should return report");

        assert_eq!(
            report.operation.last_error,
            Some("adapter execution failed".to_string())
        );
        let failed = report
            .events
            .iter()
            .find(|event| event.event_type == AuditEventType::ExecutionFailed)
            .expect("failed event should exist");
        assert_eq!(
            failed
                .execution
                .as_ref()
                .and_then(|execution| execution.message.clone()),
            Some("adapter execution failed".to_string())
        );
    }
}

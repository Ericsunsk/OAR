mod adapter;
mod policy;
mod result;
#[cfg(test)]
mod tests;
mod trace;

use std::time::{SystemTime, UNIX_EPOCH};

use crate::action::audit_event::{AuditEvent, AuditStateSummary};
use crate::action::audit_repository::{AuditEventRepository, InMemoryAuditEventRepository};
use crate::action::confirmed_action::{ActionStatus, ConfirmedAction};
use crate::action::execution_policy::{ActionActorBinding, ExecutionDenied, ExecutionPolicy};
use crate::action::execution_request::ConfirmedExecutionRequest;
use crate::action::operation_ledger::{OperationRecord, SubmitResult};
use crate::action::operation_ledger_repository::{
    InMemoryOperationLedgerRepository, OperationLedgerRepository,
};
use crate::domain::identity::TokenGrant;

pub use adapter::{AdapterDryRun, AdapterError, AdapterExecution};
use policy::is_terminal_status;
pub(crate) use policy::safe_denial_message;
pub use result::{ExecutionError, ExecutionReport, PolicyDenialReport};
pub(crate) use trace::action_audit_trace;

pub trait ActionAdapter {
    fn dry_run(
        &mut self,
        request: &ConfirmedExecutionRequest,
    ) -> Result<AdapterDryRun, AdapterError>;
    fn execute(
        &mut self,
        request: &ConfirmedExecutionRequest,
    ) -> Result<AdapterExecution, AdapterError>;
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

    pub fn execute_confirmed_request(
        &mut self,
        request: &ConfirmedExecutionRequest,
    ) -> Result<ExecutionReport, ExecutionError> {
        let action = request.action();
        match self.ledger.submit_confirmed_action(action)? {
            SubmitResult::Created(created) => self.run_from_submitted(request, created),
            SubmitResult::Existing(existing) if is_terminal_status(existing.status) => {
                Ok(self.duplicate_report(existing))
            }
            SubmitResult::Existing(existing) if existing.status == ActionStatus::Executing => {
                Ok(self.duplicate_report(existing))
            }
            SubmitResult::Existing(existing) => self.run_from_submitted(request, existing),
        }
    }

    pub fn execute_confirmed_request_with_policy(
        &mut self,
        request: &ConfirmedExecutionRequest,
        action_type: &str,
        required_scope: &str,
        actor_binding: &ActionActorBinding,
        grant: &TokenGrant,
        policy: &ExecutionPolicy,
    ) -> Result<ExecutionReport, ExecutionError> {
        let action = request.action();
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

        self.execute_confirmed_request(request)
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

    fn run_from_submitted(
        &mut self,
        request: &ConfirmedExecutionRequest,
        submitted: OperationRecord,
    ) -> Result<ExecutionReport, ExecutionError> {
        let action = request.action();
        let mut trace = action_audit_trace(action);
        let mut events = Vec::new();

        if submitted.status == ActionStatus::Confirmed {
            self.record_confirmed(action, &mut trace, &mut events)?;
            self.record_dry_run(request, &mut trace, &mut events)?;
            self.ledger.mark_executing(&action.idempotency_key)?;
        }

        let execute_result = self.adapter.execute(request);
        let operation = match execute_result {
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
                record
            }
        };

        Ok(ExecutionReport {
            operation,
            events,
            duplicate: false,
        })
    }

    fn duplicate_report(&self, operation: OperationRecord) -> ExecutionReport {
        ExecutionReport {
            operation,
            events: Vec::new(),
            duplicate: true,
        }
    }

    fn record_confirmed(
        &mut self,
        action: &ConfirmedAction,
        trace: &mut crate::action::audit_trace::AuditTrace,
        events: &mut Vec<AuditEvent>,
    ) -> Result<(), ExecutionError> {
        let confirmed_event = self.event_confirmed(trace, action);
        self.record_event(events, confirmed_event)
    }

    fn record_dry_run(
        &mut self,
        request: &ConfirmedExecutionRequest,
        trace: &mut crate::action::audit_trace::AuditTrace,
        events: &mut Vec<AuditEvent>,
    ) -> Result<(), ExecutionError> {
        let dry_run = self.adapter.dry_run(request)?;
        let dry_run_event = self.event_dry_run(trace, dry_run.before, dry_run.after);
        self.record_event(events, dry_run_event)
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

    fn event_confirmed(
        &mut self,
        trace: &mut crate::action::audit_trace::AuditTrace,
        action: &ConfirmedAction,
    ) -> AuditEvent {
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
        trace: &mut crate::action::audit_trace::AuditTrace,
        before: Option<AuditStateSummary>,
        after: Option<AuditStateSummary>,
    ) -> AuditEvent {
        let occurred_at_ms = self.now_ms();
        trace.dry_run(occurred_at_ms, before, after)
    }

    fn event_succeeded(
        &mut self,
        trace: &mut crate::action::audit_trace::AuditTrace,
        before: Option<AuditStateSummary>,
        after: Option<AuditStateSummary>,
        adapter_operation_id: String,
    ) -> AuditEvent {
        let occurred_at_ms = self.now_ms();
        trace.execution_succeeded(occurred_at_ms, before, after, adapter_operation_id)
    }

    fn event_failed(
        &mut self,
        trace: &mut crate::action::audit_trace::AuditTrace,
        error_code: String,
        message: String,
    ) -> AuditEvent {
        let occurred_at_ms = self.now_ms();
        trace.execution_failed(occurred_at_ms, None, None, error_code, message)
    }

    fn event_denied(
        &mut self,
        trace: &mut crate::action::audit_trace::AuditTrace,
        denial: &ExecutionDenied,
    ) -> AuditEvent {
        let occurred_at_ms = self.now_ms();
        trace.execution_denied(occurred_at_ms, "policy_denied", safe_denial_message(denial))
    }

    fn now_ms(&mut self) -> u64 {
        (self.clock_ms)()
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_millis() as u64)
        .unwrap_or(0)
}

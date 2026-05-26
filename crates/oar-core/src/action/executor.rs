use std::time::{SystemTime, UNIX_EPOCH};

use super::audit_event::{
    AuditActor, AuditActorKind, AuditEvent, AuditScope, AuditStateSummary, AuditTarget,
};
use super::confirmed_action::{ActionStatus, ConfirmedAction};
use super::operation_ledger::{LedgerError, OperationLedger, OperationRecord, SubmitResult};

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
    pub message: String,
}

impl AdapterError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
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
pub enum ExecutionError {
    Ledger(LedgerError),
    Adapter(AdapterError),
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

pub struct ActionExecutor<A, C = fn() -> u64>
where
    A: ActionAdapter,
    C: FnMut() -> u64,
{
    ledger: OperationLedger,
    adapter: A,
    clock_ms: C,
    sequence: u64,
}

impl<A> ActionExecutor<A, fn() -> u64>
where
    A: ActionAdapter,
{
    pub fn new(adapter: A) -> Self {
        Self::with_clock(adapter, now_ms)
    }
}

impl<A, C> ActionExecutor<A, C>
where
    A: ActionAdapter,
    C: FnMut() -> u64,
{
    pub fn with_clock(adapter: A, clock_ms: C) -> Self {
        Self {
            ledger: OperationLedger::new(),
            adapter,
            clock_ms,
            sequence: 0,
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

    pub fn ledger(&self) -> &OperationLedger {
        &self.ledger
    }

    pub fn adapter(&self) -> &A {
        &self.adapter
    }

    fn run_new_operation(
        &mut self,
        action: &ConfirmedAction,
        created: OperationRecord,
    ) -> Result<ExecutionReport, ExecutionError> {
        let mut events = Vec::new();
        events.push(self.event_confirmed(action));

        let dry_run = self.adapter.dry_run(action)?;
        events.push(self.event_dry_run(action, dry_run.before, dry_run.after));

        self.ledger.mark_executing(&action.idempotency_key)?;
        let execute_result = self.adapter.execute(action);

        let final_record = match execute_result {
            Ok(execution) => {
                let record = self.ledger.mark_succeeded(&action.idempotency_key)?;
                events.push(self.event_succeeded(
                    action,
                    execution.before,
                    execution.after,
                    execution.adapter_operation_id,
                ));
                record
            }
            Err(error) => {
                let record = self
                    .ledger
                    .mark_failed(&action.idempotency_key, error.message.clone())?;
                events.push(self.event_failed(action, error.code.clone(), error.message.clone()));
                return Ok(ExecutionReport {
                    operation: record,
                    events,
                    duplicate: false,
                });
            }
        };

        let operation = self
            .ledger
            .get_by_idempotency_key(&action.idempotency_key)
            .cloned()
            .unwrap_or(created);

        Ok(ExecutionReport {
            operation: if operation.status == ActionStatus::Succeeded {
                operation
            } else {
                final_record
            },
            events,
            duplicate: false,
        })
    }

    fn event_confirmed(&mut self, action: &ConfirmedAction) -> AuditEvent {
        AuditEvent::confirmed_action(
            self.next_event_id(),
            self.trace_id(action),
            self.next_sequence(),
            self.now_ms(),
            self.actor(action),
            self.scope(action),
            self.target(action),
            AuditStateSummary {
                summary: format!("confirmed action {}", action.action_id),
                reference_ids: vec![action.idempotency_key.clone()],
                content_hash: None,
            },
        )
    }

    fn event_dry_run(
        &mut self,
        action: &ConfirmedAction,
        before: Option<AuditStateSummary>,
        after: Option<AuditStateSummary>,
    ) -> AuditEvent {
        AuditEvent::dry_run(
            self.next_event_id(),
            self.trace_id(action),
            self.next_sequence(),
            self.now_ms(),
            self.actor(action),
            self.scope(action),
            self.target(action),
            before,
            after,
        )
    }

    fn event_succeeded(
        &mut self,
        action: &ConfirmedAction,
        before: Option<AuditStateSummary>,
        after: Option<AuditStateSummary>,
        adapter_operation_id: String,
    ) -> AuditEvent {
        AuditEvent::execution_succeeded(
            self.next_event_id(),
            self.trace_id(action),
            self.next_sequence(),
            self.now_ms(),
            self.actor(action),
            self.scope(action),
            self.target(action),
            before,
            after,
            adapter_operation_id,
        )
    }

    fn event_failed(
        &mut self,
        action: &ConfirmedAction,
        error_code: String,
        message: String,
    ) -> AuditEvent {
        AuditEvent::execution_failed(
            self.next_event_id(),
            self.trace_id(action),
            self.next_sequence(),
            self.now_ms(),
            self.actor(action),
            self.scope(action),
            self.target(action),
            None,
            None,
            error_code,
            message,
        )
    }

    fn next_event_id(&mut self) -> String {
        format!("evt-{}", self.sequence + 1)
    }

    fn next_sequence(&mut self) -> u64 {
        self.sequence += 1;
        self.sequence
    }

    fn now_ms(&mut self) -> u64 {
        (self.clock_ms)()
    }

    fn trace_id(&self, action: &ConfirmedAction) -> String {
        format!("trace-{}", action.idempotency_key)
    }

    fn actor(&self, action: &ConfirmedAction) -> AuditActor {
        AuditActor {
            kind: AuditActorKind::User,
            actor_id: action.actor_user_id.clone(),
            display_name: None,
        }
    }

    fn scope(&self, action: &ConfirmedAction) -> AuditScope {
        AuditScope {
            tenant_id: action.tenant_id.clone(),
            workspace_id: None,
        }
    }

    fn target(&self, action: &ConfirmedAction) -> AuditTarget {
        AuditTarget {
            resource_type: "confirmed_action".to_string(),
            resource_id: action.action_id.clone(),
            action_type: "execute".to_string(),
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_millis() as u64)
        .unwrap_or(0)
}

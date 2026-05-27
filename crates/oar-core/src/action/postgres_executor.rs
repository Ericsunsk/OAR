use super::audit_event::{AuditEvent, AuditStateSummary};
use super::audit_trace::AuditTrace;
use super::confirmed_action::{ActionStatus, ConfirmedAction};
use super::execution_policy::{ActionActorBinding, ExecutionDenied, ExecutionPolicy};
use super::executor::{
    action_audit_trace, safe_denial_message, ActionAdapter, AdapterError, ExecutionError,
    ExecutionReport, PolicyDenialReport,
};
use crate::domain::identity::TokenGrant;
use crate::storage::postgres::{
    AuditOutboxEnvelope, PostgresAuditEventRepository, PostgresExecutionRecorder,
};
use serde_json::json;

#[derive(Debug, Clone)]
pub struct PostgresActionExecutor<A, C = fn() -> u64>
where
    A: ActionAdapter,
    C: FnMut() -> u64,
{
    recorder: PostgresExecutionRecorder,
    audit: PostgresAuditEventRepository,
    adapter: A,
    clock_ms: C,
    outbox_stream: String,
}

impl<A, C> PostgresActionExecutor<A, C>
where
    A: ActionAdapter,
    C: FnMut() -> u64,
{
    pub fn new(
        adapter: A,
        clock_ms: C,
        recorder: PostgresExecutionRecorder,
        audit: PostgresAuditEventRepository,
    ) -> Self {
        Self {
            recorder,
            audit,
            adapter,
            clock_ms,
            outbox_stream: "audit-events".to_string(),
        }
    }

    pub async fn execute_confirmed_action(
        &mut self,
        action: &ConfirmedAction,
    ) -> Result<ExecutionReport, ExecutionError> {
        let mut trace = action_audit_trace(action);
        let mut events = Vec::new();
        let confirmed_event = self.event_confirmed(&mut trace, action);
        let confirmed_outbox = self.outbox_for(action, &confirmed_event);
        let confirmed_at_ms = action_confirmed_at_ms(action).unwrap_or_else(|| self.now_ms());
        let confirmed = self
            .recorder
            .record_confirmation(
                action,
                confirmed_at_ms,
                &operation_id(action),
                &confirmed_event,
                &confirmed_outbox,
            )
            .await
            .map_err(postgres_error_to_execution_error)?;

        if confirmed.duplicate && is_terminal_status(confirmed.operation.status) {
            return Ok(ExecutionReport {
                operation: confirmed.operation,
                events,
                duplicate: true,
            });
        }
        if confirmed.duplicate && confirmed.operation.status == ActionStatus::Executing {
            // Fail closed: without lease/timeout ownership, an existing executing record
            // means this worker did not acquire execution rights.
            return Ok(ExecutionReport {
                operation: confirmed.operation,
                events,
                duplicate: true,
            });
        }
        if !confirmed.duplicate {
            events.push(confirmed_event);
        }

        let dry_run = self.adapter.dry_run(action)?;
        let dry_run_event = self.event_dry_run(&mut trace, dry_run.before, dry_run.after);
        let dry_run_outbox = self.outbox_for(action, &dry_run_event);
        let dry_run_at_ms = self.now_ms();
        let dry_run_report = self
            .recorder
            .record_dry_run(
                &action.tenant_id,
                &action.idempotency_key,
                dry_run_at_ms,
                &dry_run_event,
                &dry_run_outbox,
            )
            .await
            .map_err(postgres_error_to_execution_error)?;
        if dry_run_report.duplicate && is_terminal_status(dry_run_report.operation.status) {
            return Ok(ExecutionReport {
                operation: dry_run_report.operation,
                events,
                duplicate: true,
            });
        }
        if dry_run_report.duplicate && dry_run_report.operation.status == ActionStatus::Executing {
            // Another worker may acquire execution rights between confirmation replay
            // and this dry-run attempt. Do not continue to write without ownership.
            return Ok(ExecutionReport {
                operation: dry_run_report.operation,
                events,
                duplicate: true,
            });
        }
        if !dry_run_report.duplicate {
            events.push(dry_run_event);
        }

        match self.adapter.execute(action) {
            Ok(execution) => {
                let succeeded_event = self.event_succeeded(
                    &mut trace,
                    execution.before,
                    execution.after,
                    execution.adapter_operation_id,
                );
                let succeeded_outbox = self.outbox_for(action, &succeeded_event);
                let succeeded_at_ms = self.now_ms();
                let report = self
                    .recorder
                    .record_success(
                        &action.tenant_id,
                        &action.idempotency_key,
                        succeeded_at_ms,
                        &succeeded_event,
                        &succeeded_outbox,
                    )
                    .await
                    .map_err(postgres_error_to_execution_error)?;
                if !report.duplicate {
                    events.push(succeeded_event);
                }
                Ok(ExecutionReport {
                    operation: report.operation,
                    events,
                    duplicate: report.duplicate,
                })
            }
            Err(error) => {
                let failed_event =
                    self.event_failed(&mut trace, error.code.clone(), error.safe_message.clone());
                let failed_outbox = self.outbox_for(action, &failed_event);
                let failed_at_ms = self.now_ms();
                let report = self
                    .recorder
                    .record_failure(
                        &action.tenant_id,
                        &action.idempotency_key,
                        &error.safe_message,
                        failed_at_ms,
                        &failed_event,
                        &failed_outbox,
                    )
                    .await
                    .map_err(postgres_error_to_execution_error)?;
                if !report.duplicate {
                    events.push(failed_event);
                }
                Ok(ExecutionReport {
                    operation: report.operation,
                    events,
                    duplicate: report.duplicate,
                })
            }
        }
    }

    pub async fn execute_confirmed_action_with_policy(
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
            self.audit
                .append(&event, None)
                .await
                .map_err(postgres_error_to_execution_error)?;
            return Err(ExecutionError::PolicyDenied(PolicyDenialReport {
                denial,
                events: vec![event],
            }));
        }

        self.execute_confirmed_action(action).await
    }

    pub fn adapter(&self) -> &A {
        &self.adapter
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

    fn outbox_for(&mut self, action: &ConfirmedAction, event: &AuditEvent) -> AuditOutboxEnvelope {
        AuditOutboxEnvelope {
            tenant_id: action.tenant_id.clone(),
            stream: self.outbox_stream.clone(),
            aggregate_id: event.trace_id.clone(),
            payload: json!({
                "event_id": event.event_id,
                "trace_id": event.trace_id,
                "event_type": format!("{:?}", event.event_type),
            }),
            next_attempt_at_ms: self.now_ms(),
        }
    }

    fn now_ms(&mut self) -> u64 {
        (self.clock_ms)()
    }
}

fn action_confirmed_at_ms(action: &ConfirmedAction) -> Option<u64> {
    let confirmed_at = action.confirmed_at?;
    confirmed_at
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis() as u64)
}

fn operation_id(action: &ConfirmedAction) -> String {
    format!("op-{}", action.idempotency_key)
}

fn is_terminal_status(status: ActionStatus) -> bool {
    matches!(
        status,
        ActionStatus::Succeeded | ActionStatus::Failed | ActionStatus::Cancelled
    )
}

fn postgres_error_to_execution_error(
    error: crate::storage::postgres::PostgresRepositoryError,
) -> ExecutionError {
    ExecutionError::Adapter(AdapterError::from_safe_message(
        "postgres_repository_error",
        error.to_string(),
    ))
}

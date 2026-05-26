use super::audit_event::{
    AuditActor, AuditActorKind, AuditEvent, AuditScope, AuditStateSummary, AuditTarget,
};
use super::confirmed_action::{ActionStatus, ConfirmedAction};
use super::execution_policy::{ExecutionDenied, ExecutionPolicy};
use super::executor::{
    safe_denial_message, ActionAdapter, AdapterError, ExecutionError, ExecutionReport,
    PolicyDenialReport,
};
use crate::domain::identity::TokenGrant;
use crate::storage::postgres::{
    AuditOutboxEnvelope, PostgresAuditEventRepository, PostgresExecutionUnitOfWork,
};
use serde_json::json;

#[derive(Debug, Clone)]
pub struct PostgresActionExecutor<A, C = fn() -> u64>
where
    A: ActionAdapter,
    C: FnMut() -> u64,
{
    uow: PostgresExecutionUnitOfWork,
    audit: PostgresAuditEventRepository,
    adapter: A,
    clock_ms: C,
    sequence: u64,
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
        uow: PostgresExecutionUnitOfWork,
        audit: PostgresAuditEventRepository,
    ) -> Self {
        Self {
            uow,
            audit,
            adapter,
            clock_ms,
            sequence: 0,
            outbox_stream: "audit-events".to_string(),
        }
    }

    pub async fn execute_confirmed_action(
        &mut self,
        action: &ConfirmedAction,
    ) -> Result<ExecutionReport, ExecutionError> {
        let mut events = Vec::new();
        let confirmed_event = self.event_confirmed(action);
        let confirmed_outbox = self.outbox_for(action, &confirmed_event);
        let confirmed_at_ms = action_confirmed_at_ms(action).unwrap_or_else(|| self.now_ms());
        let confirmed = self
            .uow
            .record_confirmation(
                action,
                confirmed_at_ms,
                &operation_id(action),
                &confirmed_event,
                &confirmed_outbox,
            )
            .await
            .map_err(postgres_error_to_execution_error)?;

        if confirmed.duplicate && confirmed.operation.status != ActionStatus::Confirmed {
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
        let dry_run_event = self.event_dry_run(action, dry_run.before, dry_run.after);
        let dry_run_outbox = self.outbox_for(action, &dry_run_event);
        let dry_run_at_ms = self.now_ms();
        let dry_run_report = self
            .uow
            .record_dry_run(
                &action.tenant_id,
                &action.idempotency_key,
                dry_run_at_ms,
                &dry_run_event,
                &dry_run_outbox,
            )
            .await
            .map_err(postgres_error_to_execution_error)?;
        if dry_run_report.duplicate {
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
                    action,
                    execution.before,
                    execution.after,
                    execution.adapter_operation_id,
                );
                let succeeded_outbox = self.outbox_for(action, &succeeded_event);
                let succeeded_at_ms = self.now_ms();
                let report = self
                    .uow
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
                    duplicate: false,
                })
            }
            Err(error) => {
                let failed_event =
                    self.event_failed(action, error.code.clone(), error.message.clone());
                let failed_outbox = self.outbox_for(action, &failed_event);
                let failed_at_ms = self.now_ms();
                let report = self
                    .uow
                    .record_failure(
                        &action.tenant_id,
                        &action.idempotency_key,
                        &error.message,
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
                    duplicate: false,
                })
            }
        }
    }

    pub async fn execute_confirmed_action_with_policy(
        &mut self,
        action: &ConfirmedAction,
        action_type: &str,
        required_scope: &str,
        grant: &TokenGrant,
        policy: &ExecutionPolicy,
    ) -> Result<ExecutionReport, ExecutionError> {
        if let Err(denial) = policy.evaluate(action, action_type, required_scope, grant) {
            let event = self.event_denied(action, &denial);
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

    fn event_confirmed(&mut self, action: &ConfirmedAction) -> AuditEvent {
        AuditEvent::confirmed_action(
            self.next_event_id(action),
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
            self.next_event_id(action),
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
            self.next_event_id(action),
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
            self.next_event_id(action),
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

    fn event_denied(&mut self, action: &ConfirmedAction, denial: &ExecutionDenied) -> AuditEvent {
        AuditEvent::execution_denied(
            self.next_event_id(action),
            self.trace_id(action),
            self.next_sequence(),
            self.now_ms(),
            self.actor(action),
            self.scope(action),
            self.target(action),
            "policy_denied",
            safe_denial_message(denial),
        )
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

    fn next_event_id(&self, action: &ConfirmedAction) -> String {
        format!("{}-evt-{}", self.trace_id(action), self.sequence + 1)
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

fn postgres_error_to_execution_error(
    error: crate::storage::postgres::PostgresRepositoryError,
) -> ExecutionError {
    ExecutionError::Adapter(AdapterError::new(
        "postgres_repository_error",
        error.to_string(),
    ))
}

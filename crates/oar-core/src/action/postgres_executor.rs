use super::audit_event::AuditEvent;
use super::confirmed_action::{ActionStatus, ConfirmedAction};
use super::execution_policy::{ActionActorBinding, ExecutionPolicy};
use super::execution_request::ConfirmedExecutionRequest;
use super::executor::{
    action_audit_trace, events as audit_events, ActionAdapter, AdapterError, ExecutionError,
    ExecutionReport, PolicyDenialReport,
};
use crate::domain::identity::TokenGrant;
use crate::storage::postgres::{
    postgres_repository_safe_error_reason, AuditOutboxEnvelope, PostgresAuditEventRepository,
    PostgresExecutionRecorder, PostgresRepositoryError,
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

    pub async fn execute_confirmed_request(
        &mut self,
        request: &ConfirmedExecutionRequest,
    ) -> Result<ExecutionReport, ExecutionError> {
        let action = request.action();
        let mut trace = action_audit_trace(action);
        let mut events = Vec::new();
        let confirmed_event = audit_events::confirmed_action(self.now_ms(), &mut trace, action);
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

        let dry_run = self.adapter.dry_run(request)?;
        let dry_run_event =
            audit_events::dry_run(self.now_ms(), &mut trace, dry_run.before, dry_run.after);
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

        match self.adapter.execute(request) {
            Ok(execution) => {
                let succeeded_event = audit_events::execution_succeeded(
                    self.now_ms(),
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
                let failed_event = audit_events::execution_failed(
                    self.now_ms(),
                    &mut trace,
                    error.code.clone(),
                    error.safe_message.clone(),
                );
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

    pub async fn execute_confirmed_request_with_policy(
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
            let event = audit_events::execution_denied(self.now_ms(), &mut trace, &denial);
            self.audit
                .append(&event, None)
                .await
                .map_err(postgres_error_to_execution_error)?;
            return Err(ExecutionError::PolicyDenied(PolicyDenialReport {
                denial,
                events: vec![event],
            }));
        }

        self.execute_confirmed_request(request).await
    }

    pub fn adapter(&self) -> &A {
        &self.adapter
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

fn postgres_error_to_execution_error(error: PostgresRepositoryError) -> ExecutionError {
    ExecutionError::Adapter(AdapterError::from_safe_message(
        "postgres_repository_error",
        postgres_repository_safe_error_reason(&error),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postgres_error_to_execution_error_redacts_raw_repository_text() {
        let error = PostgresRepositoryError::UnknownTenantStatus(
            "raw tenant status with password".to_string(),
        );

        let mapped = postgres_error_to_execution_error(error);

        let ExecutionError::Adapter(error) = mapped else {
            panic!("postgres repository failures should map to adapter errors");
        };
        assert_eq!(error.code, "postgres_repository_error");
        assert_eq!(error.safe_message, "unknown_tenant_status");
        assert!(!error.safe_message.contains("password"));
        assert!(!error.safe_message.contains("raw tenant status"));
    }
}

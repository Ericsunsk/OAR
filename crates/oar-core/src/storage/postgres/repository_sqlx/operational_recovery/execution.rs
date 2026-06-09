use super::super::*;
use crate::action::audit_event::{
    AuditActor, AuditActorKind, AuditStateSummary, AuditSubject, AuditTarget,
};
use crate::action::audit_trace::AuditTrace;

mod outbox_requeue;
mod resume;

impl PostgresOperationalRecoveryRepository {
    pub async fn execute_confirmed_recovery(
        &self,
        request: PostgresOperationalRecoveryExecutionRequest,
    ) -> PgRepositoryResult<PostgresOperationalRecoveryExecutionReport> {
        let mut tx = self.pool.begin().await?;
        let submit = super::super::action_execution::submit_confirmed_action_in_tx(
            &mut tx,
            &request.action,
            request.confirmed_at_ms,
            &request.operation_id,
        )
        .await?;
        let (operation, duplicate) = recovery_submit_result_parts(submit);
        if duplicate && is_recovery_terminal_or_inflight(operation.status) {
            tx.commit().await?;
            return Ok(PostgresOperationalRecoveryExecutionReport {
                operation,
                duplicate: true,
                recovered_target: None,
                events: Vec::new(),
            });
        }

        let mut events = Vec::new();
        let mut trace = recovery_audit_trace(&request);
        if !duplicate {
            let event = trace.confirmed_action(
                request.occurred_at_ms,
                AuditStateSummary {
                    summary: format!(
                        "confirmed operational recovery {}",
                        request.action.action_id
                    ),
                    reference_ids: request.kind.target_reference_ids(),
                    content_hash: None,
                },
            );
            append_recovery_event_in_tx(&mut tx, &event, &operation.operation_id, &request).await?;
            events.push(event);
        }

        let (operation, duplicate_executing) = super::super::action_execution::transition_in_tx(
            &mut tx,
            super::super::action_execution::OperationStatusTransition::mark_executing(),
            &request.action.tenant_id,
            &request.action.idempotency_key,
            None,
            request.occurred_at_ms,
        )
        .await?;
        if duplicate_executing {
            tx.commit().await?;
            return Ok(PostgresOperationalRecoveryExecutionReport {
                operation,
                duplicate: true,
                recovered_target: None,
                events,
            });
        }

        match request.kind.clone() {
            OperationalRecoveryExecutionKind::ResumePausedAuthRefresh {
                grant_id,
                expected_updated_at_ms,
            } => {
                resume::execute_resume_paused_auth_refresh(
                    tx,
                    request,
                    operation,
                    events,
                    trace,
                    &grant_id,
                    expected_updated_at_ms,
                )
                .await
            }
            OperationalRecoveryExecutionKind::RequeueFailedAuditOutbox {
                outbox_id,
                expected_attempt_count,
                requeue_next_attempt_at_ms,
            } => {
                outbox_requeue::execute_requeue_failed_audit_outbox(
                    tx,
                    request,
                    operation,
                    events,
                    trace,
                    outbox_id,
                    expected_attempt_count,
                    requeue_next_attempt_at_ms,
                )
                .await
            }
        }
    }
}

async fn fail_recovery_operation(
    tx: &mut Transaction<'_, Postgres>,
    request: &PostgresOperationalRecoveryExecutionRequest,
    mut events: Vec<AuditEvent>,
    mut trace: AuditTrace,
    error_code: &str,
    message: &str,
) -> PgRepositoryResult<(OperationRecord, Vec<AuditEvent>)> {
    let failed_event = recovery_failed_event(&mut trace, request, error_code, message);
    let (operation, _) = super::super::action_execution::transition_in_tx(
        tx,
        super::super::action_execution::OperationStatusTransition::mark_failed(),
        &request.action.tenant_id,
        &request.action.idempotency_key,
        Some(error_code),
        request.occurred_at_ms,
    )
    .await?;
    append_recovery_event_in_tx(tx, &failed_event, &operation.operation_id, request).await?;
    events.push(failed_event);

    Ok((operation, events))
}

fn recovery_submit_result_parts(result: SubmitResult) -> (OperationRecord, bool) {
    match result {
        SubmitResult::Created(record) => (record, false),
        SubmitResult::Existing(record) => (record, true),
    }
}

fn is_recovery_terminal_or_inflight(status: ActionStatus) -> bool {
    matches!(
        status,
        ActionStatus::Executing
            | ActionStatus::Succeeded
            | ActionStatus::Failed
            | ActionStatus::Cancelled
    )
}

fn recovery_audit_trace(request: &PostgresOperationalRecoveryExecutionRequest) -> AuditTrace {
    AuditTrace::new(
        request.audit_trace_id.clone(),
        AuditSubject {
            actor: AuditActor {
                kind: AuditActorKind::User,
                actor_id: request.action.actor_user_id.clone(),
                display_name: None,
            },
            scope: AuditScope {
                tenant_id: request.action.tenant_id.clone(),
                workspace_id: None,
            },
            target: AuditTarget {
                resource_type: "operational_recovery".to_string(),
                resource_id: request.action.action_id.clone(),
                action_type: request.kind.action_type().to_string(),
            },
        },
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RecoveryDryRunOutcome {
    Eligible(String),
    UnsafePayload(String),
    Missing,
}

fn recovery_dry_run_event(
    trace: &mut AuditTrace,
    request: &PostgresOperationalRecoveryExecutionRequest,
    outcome: RecoveryDryRunOutcome,
) -> AuditEvent {
    let reference_ids = request.kind.target_reference_ids();
    let after_summary = match (&request.kind, outcome) {
        (
            OperationalRecoveryExecutionKind::ResumePausedAuthRefresh { .. },
            RecoveryDryRunOutcome::Eligible(reference_id),
        ) => AuditStateSummary {
            summary: "paused auth refresh grant is eligible for recovery resume".to_string(),
            reference_ids: vec![reference_id],
            content_hash: None,
        },
        (
            OperationalRecoveryExecutionKind::ResumePausedAuthRefresh { .. },
            RecoveryDryRunOutcome::UnsafePayload(reference_id),
        ) => AuditStateSummary {
            summary: "paused auth refresh grant matched an unsafe recovery state".to_string(),
            reference_ids: vec![reference_id],
            content_hash: None,
        },
        (
            OperationalRecoveryExecutionKind::ResumePausedAuthRefresh { .. },
            RecoveryDryRunOutcome::Missing,
        ) => AuditStateSummary {
            summary: "no eligible paused auth refresh grant matched the recovery request"
                .to_string(),
            reference_ids,
            content_hash: None,
        },
        (
            OperationalRecoveryExecutionKind::RequeueFailedAuditOutbox { .. },
            RecoveryDryRunOutcome::Eligible(reference_id),
        ) => AuditStateSummary {
            summary: "failed audit outbox is eligible for recovery requeue".to_string(),
            reference_ids: vec![reference_id],
            content_hash: None,
        },
        (
            OperationalRecoveryExecutionKind::RequeueFailedAuditOutbox { .. },
            RecoveryDryRunOutcome::UnsafePayload(reference_id),
        ) => AuditStateSummary {
            summary: "failed audit outbox matched but payload did not pass safety validation"
                .to_string(),
            reference_ids: vec![reference_id],
            content_hash: None,
        },
        (
            OperationalRecoveryExecutionKind::RequeueFailedAuditOutbox { .. },
            RecoveryDryRunOutcome::Missing,
        ) => AuditStateSummary {
            summary: "no eligible failed audit outbox matched the recovery request".to_string(),
            reference_ids,
            content_hash: None,
        },
    };
    trace.dry_run(
        request.occurred_at_ms,
        Some(AuditStateSummary {
            summary: "operational recovery request re-read live state before write".to_string(),
            reference_ids: request.kind.target_reference_ids(),
            content_hash: None,
        }),
        Some(after_summary),
    )
}

fn recovery_succeeded_event(
    trace: &mut AuditTrace,
    request: &PostgresOperationalRecoveryExecutionRequest,
    target: OperationalRecoveryExecutionTarget,
) -> AuditEvent {
    match target {
        OperationalRecoveryExecutionTarget::TokenGrantRefresh { grant_id } => trace
            .execution_succeeded(
            request.occurred_at_ms,
            Some(AuditStateSummary {
                summary: "paused auth refresh grant had a recoverable safe refresh blocker"
                    .to_string(),
                reference_ids: vec![grant_id.clone()],
                content_hash: None,
            }),
            Some(AuditStateSummary {
                summary:
                    "paused auth refresh blocker was cleared; scheduler can re-evaluate the grant"
                        .to_string(),
                reference_ids: vec![grant_id.clone()],
                content_hash: None,
            }),
            format!("operational-recovery:resume-paused-auth-refresh:{grant_id}"),
        ),
        OperationalRecoveryExecutionTarget::AuditOutboxRequeue { outbox_id } => trace
            .execution_succeeded(
            request.occurred_at_ms,
            Some(AuditStateSummary {
                summary: "failed audit outbox had a recoverable safe delivery blocker".to_string(),
                reference_ids: vec![format!("audit_outbox:{outbox_id}")],
                content_hash: None,
            }),
            Some(AuditStateSummary {
                summary:
                    "failed audit outbox was reopened for delivery; worker can retry the same row"
                        .to_string(),
                reference_ids: vec![format!("audit_outbox:{outbox_id}")],
                content_hash: None,
            }),
            format!("operational-recovery:requeue-failed-audit-outbox:{outbox_id}"),
        ),
    }
}

fn recovery_failed_event(
    trace: &mut AuditTrace,
    request: &PostgresOperationalRecoveryExecutionRequest,
    error_code: &str,
    message: &str,
) -> AuditEvent {
    trace.execution_failed(
        request.occurred_at_ms,
        Some(AuditStateSummary {
            summary: "operational recovery request re-read live state before write".to_string(),
            reference_ids: request.kind.target_reference_ids(),
            content_hash: None,
        }),
        None,
        error_code,
        message,
    )
}

async fn append_recovery_event_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    event: &AuditEvent,
    operation_id: &str,
    request: &PostgresOperationalRecoveryExecutionRequest,
) -> PgRepositoryResult<()> {
    super::super::audit::append_audit_event_in_tx(tx, event, Some(operation_id)).await?;
    let outbox = AuditOutboxEnvelope {
        tenant_id: request.action.tenant_id.clone(),
        stream: "audit-events".to_string(),
        aggregate_id: event.trace_id.clone(),
        payload: serde_json::json!({
            "event_id": event.event_id,
            "trace_id": event.trace_id,
            "event_type": format!("{:?}", event.event_type),
            "kind": "operational_recovery",
        }),
        next_attempt_at_ms: request.outbox_next_attempt_at_ms,
    };
    super::super::audit::enqueue_outbox_in_tx(tx, &outbox).await?;
    Ok(())
}

use super::super::*;
use crate::action::audit_event::{
    AuditActor, AuditActorKind, AuditStateSummary, AuditSubject, AuditTarget,
};
use crate::action::audit_trace::AuditTrace;

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
                resumed_token_grant_id: None,
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
                resumed_token_grant_id: None,
                events,
            });
        }

        match request.kind.clone() {
            OperationalRecoveryExecutionKind::ResumePausedAuthRefresh {
                grant_id,
                expected_updated_at_ms,
            } => {
                execute_resume_paused_auth_refresh(
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
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LockedPausedTokenGrantRefresh {
    id: String,
    updated_at_ms: u64,
}

async fn execute_resume_paused_auth_refresh(
    mut tx: Transaction<'_, Postgres>,
    request: PostgresOperationalRecoveryExecutionRequest,
    operation: OperationRecord,
    mut events: Vec<AuditEvent>,
    mut trace: AuditTrace,
    grant_id: &str,
    expected_updated_at_ms: u64,
) -> PgRepositoryResult<PostgresOperationalRecoveryExecutionReport> {
    let eligible = lock_paused_token_grant_refresh_for_recovery(
        &mut tx,
        &request.action.tenant_id,
        grant_id,
        expected_updated_at_ms,
    )
    .await?;
    let dry_run_event =
        recovery_dry_run_event(&mut trace, &request, eligible.as_ref().map(|item| &item.id));
    append_recovery_event_in_tx(&mut tx, &dry_run_event, &operation.operation_id, &request).await?;
    events.push(dry_run_event);

    let Some(eligible) = eligible else {
        let (operation, events) = fail_recovery_operation(
            &mut tx,
            &request,
            events,
            trace,
            "operational_recovery_no_eligible_grant",
            "no eligible paused auth refresh grant matched the confirmed recovery request",
        )
        .await?;
        tx.commit().await?;
        return Ok(PostgresOperationalRecoveryExecutionReport {
            operation,
            duplicate: false,
            resumed_token_grant_id: None,
            events,
        });
    };

    let resumed = resume_paused_token_grant_refresh_for_recovery(
        &mut tx,
        &request.action.tenant_id,
        &eligible.id,
        eligible.updated_at_ms,
        request.occurred_at_ms,
    )
    .await?;
    let Some(resumed_id) = resumed else {
        let (operation, events) = fail_recovery_operation(
            &mut tx,
            &request,
            events,
            trace,
            "operational_recovery_resume_conflict",
            "paused auth refresh grant changed before recovery could resume it",
        )
        .await?;
        tx.commit().await?;
        return Ok(PostgresOperationalRecoveryExecutionReport {
            operation,
            duplicate: false,
            resumed_token_grant_id: None,
            events,
        });
    };

    let (operation, _) = super::super::action_execution::transition_in_tx(
        &mut tx,
        super::super::action_execution::OperationStatusTransition::mark_succeeded(),
        &request.action.tenant_id,
        &request.action.idempotency_key,
        None,
        request.occurred_at_ms,
    )
    .await?;
    let succeeded_event = recovery_succeeded_event(&mut trace, &request, &resumed_id);
    append_recovery_event_in_tx(&mut tx, &succeeded_event, &operation.operation_id, &request)
        .await?;
    tx.commit().await?;
    events.push(succeeded_event);

    Ok(PostgresOperationalRecoveryExecutionReport {
        operation,
        duplicate: false,
        resumed_token_grant_id: Some(resumed_id),
        events,
    })
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

async fn lock_paused_token_grant_refresh_for_recovery(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: &str,
    grant_id: &str,
    expected_updated_at_ms: u64,
) -> PgRepositoryResult<Option<LockedPausedTokenGrantRefresh>> {
    let row = sqlx::query(LOCK_PAUSED_TOKEN_GRANT_REFRESH_FOR_RECOVERY)
        .bind(tenant_id)
        .bind(grant_id)
        .bind(expected_updated_at_ms as i64)
        .fetch_optional(&mut **tx)
        .await?;
    row.as_ref()
        .map(|row| {
            Ok(LockedPausedTokenGrantRefresh {
                id: row.try_get("id")?,
                updated_at_ms: non_negative_i64_to_u64(
                    row.try_get("updated_at_ms")?,
                    "updated_at_ms",
                )?,
            })
        })
        .transpose()
}

async fn resume_paused_token_grant_refresh_for_recovery(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: &str,
    grant_id: &str,
    expected_updated_at_ms: u64,
    resumed_at_ms: u64,
) -> PgRepositoryResult<Option<String>> {
    let row = sqlx::query(RESUME_PAUSED_TOKEN_GRANT_REFRESH_FOR_RECOVERY)
        .bind(tenant_id)
        .bind(grant_id)
        .bind(expected_updated_at_ms as i64)
        .bind(resumed_at_ms as i64)
        .fetch_optional(&mut **tx)
        .await?;
    row.as_ref()
        .map(|row| row.try_get("id").map_err(PostgresRepositoryError::from))
        .transpose()
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

fn recovery_dry_run_event(
    trace: &mut AuditTrace,
    request: &PostgresOperationalRecoveryExecutionRequest,
    eligible_grant_id: Option<&String>,
) -> AuditEvent {
    let reference_ids = request.kind.target_reference_ids();
    let after_summary = match eligible_grant_id {
        Some(grant_id) => AuditStateSummary {
            summary: "paused auth refresh grant is eligible for recovery resume".to_string(),
            reference_ids: vec![grant_id.clone()],
            content_hash: None,
        },
        None => AuditStateSummary {
            summary: "no eligible paused auth refresh grant matched the recovery request"
                .to_string(),
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
    grant_id: &str,
) -> AuditEvent {
    trace.execution_succeeded(
        request.occurred_at_ms,
        Some(AuditStateSummary {
            summary: "paused auth refresh grant had a recoverable safe refresh blocker".to_string(),
            reference_ids: vec![grant_id.to_string()],
            content_hash: None,
        }),
        Some(AuditStateSummary {
            summary: "paused auth refresh blocker was cleared; scheduler can re-evaluate the grant"
                .to_string(),
            reference_ids: vec![grant_id.to_string()],
            content_hash: None,
        }),
        format!("operational-recovery:resume-paused-auth-refresh:{grant_id}"),
    )
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

use super::*;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
struct LockedFailedAuditOutboxRecovery {
    id: i64,
    payload: Value,
    attempt_count: i32,
}

pub(super) async fn execute_requeue_failed_audit_outbox(
    mut tx: Transaction<'_, Postgres>,
    request: PostgresOperationalRecoveryExecutionRequest,
    operation: OperationRecord,
    mut events: Vec<AuditEvent>,
    mut trace: AuditTrace,
    outbox_id: i64,
    expected_attempt_count: i32,
    requeue_next_attempt_at_ms: u64,
) -> PgRepositoryResult<PostgresOperationalRecoveryExecutionReport> {
    let eligible = lock_failed_audit_outbox_for_recovery(
        &mut tx,
        &request.action.tenant_id,
        outbox_id,
        expected_attempt_count,
    )
    .await?;
    let payload_safe = eligible.as_ref().map(|item| {
        super::super::super::audit::validate_audit_outbox_payload(&item.payload).is_ok()
    });
    let outcome = match eligible.as_ref() {
        Some(_) => {
            if payload_safe == Some(true) {
                RecoveryDryRunOutcome::Eligible(format!("audit_outbox:{outbox_id}"))
            } else {
                RecoveryDryRunOutcome::UnsafePayload(format!("audit_outbox:{outbox_id}"))
            }
        }
        None => RecoveryDryRunOutcome::Missing,
    };
    let dry_run_event = recovery_dry_run_event(&mut trace, &request, outcome.clone());
    append_recovery_event_in_tx(&mut tx, &dry_run_event, &operation.operation_id, &request).await?;
    events.push(dry_run_event);

    let Some(eligible) = eligible else {
        let (operation, events) = fail_recovery_operation(
            &mut tx,
            &request,
            events,
            trace,
            "operational_recovery_no_eligible_outbox",
            "no eligible failed audit outbox matched the confirmed recovery request",
        )
        .await?;
        tx.commit().await?;
        return Ok(PostgresOperationalRecoveryExecutionReport {
            operation,
            duplicate: false,
            recovered_target: None,
            events,
        });
    };

    if payload_safe != Some(true) {
        let (operation, events) = fail_recovery_operation(
            &mut tx,
            &request,
            events,
            trace,
            "operational_recovery_unsafe_outbox_payload",
            "failed audit outbox payload did not pass safety validation",
        )
        .await?;
        tx.commit().await?;
        return Ok(PostgresOperationalRecoveryExecutionReport {
            operation,
            duplicate: false,
            recovered_target: None,
            events,
        });
    }

    let reopened = requeue_failed_audit_outbox_for_recovery(
        &mut tx,
        &request.action.tenant_id,
        eligible.id,
        eligible.attempt_count,
        requeue_next_attempt_at_ms,
    )
    .await?;
    let Some(reopened_id) = reopened else {
        let (operation, events) = fail_recovery_operation(
            &mut tx,
            &request,
            events,
            trace,
            "operational_recovery_outbox_conflict",
            "failed audit outbox changed before recovery could requeue it",
        )
        .await?;
        tx.commit().await?;
        return Ok(PostgresOperationalRecoveryExecutionReport {
            operation,
            duplicate: false,
            recovered_target: None,
            events,
        });
    };

    let (operation, _) = super::super::super::action_execution::transition_in_tx(
        &mut tx,
        super::super::super::action_execution::OperationStatusTransition::mark_succeeded(),
        &request.action.tenant_id,
        &request.action.idempotency_key,
        None,
        request.occurred_at_ms,
    )
    .await?;
    let recovered_target = OperationalRecoveryExecutionTarget::AuditOutboxRequeue {
        outbox_id: reopened_id,
    };
    let succeeded_event = recovery_succeeded_event(&mut trace, &request, recovered_target.clone());
    append_recovery_event_in_tx(&mut tx, &succeeded_event, &operation.operation_id, &request)
        .await?;
    tx.commit().await?;
    events.push(succeeded_event);

    Ok(PostgresOperationalRecoveryExecutionReport {
        operation,
        duplicate: false,
        recovered_target: Some(recovered_target),
        events,
    })
}

async fn lock_failed_audit_outbox_for_recovery(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: &str,
    outbox_id: i64,
    expected_attempt_count: i32,
) -> PgRepositoryResult<Option<LockedFailedAuditOutboxRecovery>> {
    let row = sqlx::query(LOCK_FAILED_AUDIT_OUTBOX_FOR_RECOVERY)
        .bind(tenant_id)
        .bind(outbox_id)
        .bind(expected_attempt_count)
        .fetch_optional(&mut **tx)
        .await?;
    row.as_ref()
        .map(|row| {
            Ok(LockedFailedAuditOutboxRecovery {
                id: row.try_get("id")?,
                payload: row.try_get("payload")?,
                attempt_count: row.try_get("attempt_count")?,
            })
        })
        .transpose()
}

async fn requeue_failed_audit_outbox_for_recovery(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: &str,
    outbox_id: i64,
    expected_attempt_count: i32,
    requeue_next_attempt_at_ms: u64,
) -> PgRepositoryResult<Option<i64>> {
    let row = sqlx::query(REQUEUE_FAILED_AUDIT_OUTBOX_FOR_RECOVERY)
        .bind(tenant_id)
        .bind(outbox_id)
        .bind(expected_attempt_count)
        .bind(requeue_next_attempt_at_ms as i64)
        .fetch_optional(&mut **tx)
        .await?;
    row.as_ref()
        .map(|row| row.try_get("id").map_err(PostgresRepositoryError::from))
        .transpose()
}

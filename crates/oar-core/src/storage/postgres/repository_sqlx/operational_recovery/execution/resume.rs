use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
struct LockedPausedTokenGrantRefresh {
    id: String,
    updated_at_ms: u64,
}

pub(super) async fn execute_resume_paused_auth_refresh(
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
    let dry_run_event = recovery_dry_run_event(
        &mut trace,
        &request,
        match eligible.as_ref() {
            Some(item) => RecoveryDryRunOutcome::Eligible(item.id.clone()),
            None => RecoveryDryRunOutcome::Missing,
        },
    );
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
            recovered_target: None,
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
    let recovered_target = OperationalRecoveryExecutionTarget::TokenGrantRefresh {
        grant_id: resumed_id,
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

use super::super::*;

impl PostgresOperationalRecoveryRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn load_tenant_recovery_report(
        &self,
        tenant_id: &str,
        limit: u32,
    ) -> PgRepositoryResult<PostgresOperationalRecoveryReport> {
        if limit == 0 {
            return Ok(PostgresOperationalRecoveryReport {
                tenant_id: tenant_id.to_string(),
                failed_audit_outbox: Vec::new(),
                parked_token_grants: Vec::new(),
            });
        }

        let failed_outbox_rows = sqlx::query(LIST_FAILED_AUDIT_OUTBOX_RECOVERY_ITEMS)
            .bind(tenant_id)
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await?;
        let parked_grant_rows = sqlx::query(LIST_PARKED_TOKEN_GRANT_RECOVERY_ITEMS)
            .bind(tenant_id)
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await?;

        Ok(PostgresOperationalRecoveryReport {
            tenant_id: tenant_id.to_string(),
            failed_audit_outbox: failed_outbox_rows
                .iter()
                .map(failed_outbox_recovery_item_from_row)
                .collect::<PgRepositoryResult<Vec<_>>>()?,
            parked_token_grants: parked_grant_rows
                .iter()
                .map(parked_token_grant_recovery_item_from_row)
                .collect::<PgRepositoryResult<Vec<_>>>()?,
        })
    }
}

fn failed_outbox_recovery_item_from_row(
    row: &sqlx::postgres::PgRow,
) -> PgRepositoryResult<FailedAuditOutboxRecoveryItem> {
    let payload: Value = row.try_get("payload")?;
    let payload = SafeAuditOutboxPayload::try_from(&payload).ok();
    let payload_safe = payload.is_some();
    let stream = row.try_get("stream")?;
    let can_requeue = payload_safe && stream == "audit-events";

    Ok(FailedAuditOutboxRecoveryItem {
        id: row.try_get("id")?,
        tenant_id: row.try_get("tenant_id")?,
        stream,
        aggregate_id: row.try_get("aggregate_id")?,
        attempt_count: row.try_get("attempt_count")?,
        created_at_ms: non_negative_i64_to_u64(row.try_get("created_at_ms")?, "created_at_ms")?,
        payload,
        payload_safe,
        recommended_action: if can_requeue {
            OperationalRecoveryAction::RequeueFailedAuditOutbox
        } else {
            OperationalRecoveryAction::InspectFailedAuditOutbox
        },
    })
}

fn parked_token_grant_recovery_item_from_row(
    row: &sqlx::postgres::PgRow,
) -> PgRepositoryResult<ParkedTokenGrantRecoveryItem> {
    let state = token_grant_state_from_db(row.try_get("state")?)?;
    let safe_error = row
        .try_get::<Option<String>, _>("last_refresh_error")?
        .map(|error| sanitize_refresh_error_for_storage(&error));
    let recommended_action = match state {
        TokenGrantState::ReauthRequired => OperationalRecoveryAction::AskUserToReauthorize,
        _ => OperationalRecoveryAction::FixFeishuRefreshConfigThenResume,
    };

    Ok(ParkedTokenGrantRecoveryItem {
        grant_id: row.try_get("id")?,
        tenant_id: row.try_get("tenant_id")?,
        identity_id: row.try_get("identity_id")?,
        actor_kind: identity_actor_kind_from_db(row.try_get("actor_kind")?)?,
        scope_boundary: scope_boundary_from_db(row.try_get("scope_boundary")?)?,
        state,
        safe_error,
        refreshed_at_ms: optional_non_negative_i64_to_u64(
            row.try_get("refreshed_at_ms")?,
            "refreshed_at_ms",
        )?,
        reauth_required_at_ms: optional_non_negative_i64_to_u64(
            row.try_get("reauth_required_at_ms")?,
            "reauth_required_at_ms",
        )?,
        updated_at_ms: non_negative_i64_to_u64(row.try_get("updated_at_ms")?, "updated_at_ms")?,
        recommended_action,
    })
}

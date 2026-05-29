use super::*;

impl PostgresAuditEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn append(
        &self,
        event: &AuditEvent,
        operation_id: Option<&str>,
    ) -> PgRepositoryResult<()> {
        append_audit_event_query(event, operation_id)?
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn find_by_tenant_and_trace_id(
        &self,
        tenant_id: &str,
        trace_id: &str,
    ) -> PgRepositoryResult<Vec<AuditEvent>> {
        let rows = sqlx::query(FIND_AUDIT_EVENTS_BY_TRACE_ID)
            .bind(tenant_id)
            .bind(trace_id)
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(audit_event_from_row).collect()
    }

    pub async fn enqueue_outbox(
        &self,
        tenant_id: &str,
        stream: &str,
        aggregate_id: &str,
        payload: &Value,
        next_attempt_at_ms: u64,
    ) -> PgRepositoryResult<i64> {
        super::audit::validate_audit_outbox_payload(payload)?;
        let row =
            enqueue_outbox_query(tenant_id, stream, aggregate_id, payload, next_attempt_at_ms)
                .fetch_one(&self.pool)
                .await?;
        Ok(row.try_get("id")?)
    }

    pub async fn claim_outbox(
        &self,
        tenant_id: &str,
        stream: &str,
        now_ms: u64,
        limit: i64,
        lease_until_ms: u64,
    ) -> PgRepositoryResult<Vec<AuditOutboxMessage>> {
        let rows = sqlx::query(CLAIM_AUDIT_OUTBOX)
            .bind(tenant_id)
            .bind(stream)
            .bind(now_ms as i64)
            .bind(limit)
            .bind(lease_until_ms as i64)
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(audit_outbox_message_from_row).collect()
    }

    pub async fn mark_outbox_sent(
        &self,
        tenant_id: &str,
        id: i64,
        sent_at_ms: u64,
    ) -> PgRepositoryResult<bool> {
        let row = sqlx::query(MARK_AUDIT_OUTBOX_SENT)
            .bind(tenant_id)
            .bind(id)
            .bind(sent_at_ms as i64)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.is_some())
    }

    pub async fn mark_outbox_sent_for_attempt(
        &self,
        tenant_id: &str,
        id: i64,
        attempt_count: i32,
        lease_until_ms: u64,
        sent_at_ms: u64,
    ) -> PgRepositoryResult<bool> {
        let row = sqlx::query(MARK_AUDIT_OUTBOX_SENT_FOR_ATTEMPT)
            .bind(tenant_id)
            .bind(id)
            .bind(attempt_count)
            .bind(lease_until_ms as i64)
            .bind(sent_at_ms as i64)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.is_some())
    }

    pub async fn mark_outbox_retryable(
        &self,
        tenant_id: &str,
        id: i64,
        next_attempt_at_ms: u64,
    ) -> PgRepositoryResult<bool> {
        let row = sqlx::query(MARK_AUDIT_OUTBOX_RETRYABLE)
            .bind(tenant_id)
            .bind(id)
            .bind(next_attempt_at_ms as i64)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.is_some())
    }

    pub async fn mark_outbox_retryable_for_attempt(
        &self,
        tenant_id: &str,
        id: i64,
        attempt_count: i32,
        lease_until_ms: u64,
        next_attempt_at_ms: u64,
    ) -> PgRepositoryResult<bool> {
        let row = sqlx::query(MARK_AUDIT_OUTBOX_RETRYABLE_FOR_ATTEMPT)
            .bind(tenant_id)
            .bind(id)
            .bind(attempt_count)
            .bind(lease_until_ms as i64)
            .bind(next_attempt_at_ms as i64)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.is_some())
    }

    pub async fn mark_outbox_failed(&self, tenant_id: &str, id: i64) -> PgRepositoryResult<bool> {
        let row = sqlx::query(MARK_AUDIT_OUTBOX_FAILED)
            .bind(tenant_id)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.is_some())
    }

    pub async fn mark_outbox_failed_for_attempt(
        &self,
        tenant_id: &str,
        id: i64,
        attempt_count: i32,
        lease_until_ms: u64,
    ) -> PgRepositoryResult<bool> {
        let row = sqlx::query(MARK_AUDIT_OUTBOX_FAILED_FOR_ATTEMPT)
            .bind(tenant_id)
            .bind(id)
            .bind(attempt_count)
            .bind(lease_until_ms as i64)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.is_some())
    }
}

pub(super) async fn append_audit_event_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    event: &AuditEvent,
    operation_id: Option<&str>,
) -> PgRepositoryResult<()> {
    append_audit_event_query(event, operation_id)?
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(super) async fn enqueue_outbox_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    outbox: &AuditOutboxEnvelope,
) -> PgRepositoryResult<i64> {
    super::audit::validate_audit_outbox_payload(&outbox.payload)?;
    let row = enqueue_outbox_query(
        &outbox.tenant_id,
        &outbox.stream,
        &outbox.aggregate_id,
        &outbox.payload,
        outbox.next_attempt_at_ms,
    )
    .fetch_one(&mut **tx)
    .await?;
    Ok(row.try_get("id")?)
}

fn append_audit_event_query<'a>(
    event: &'a AuditEvent,
    operation_id: Option<&'a str>,
) -> PgRepositoryResult<sqlx::query::Query<'a, Postgres, sqlx::postgres::PgArguments>> {
    Ok(sqlx::query(APPEND_AUDIT_EVENT)
        .bind(&event.event_id)
        .bind(&event.trace_id)
        .bind(event.sequence as i64)
        .bind(event.occurred_at_ms as i64)
        .bind(&event.scope.tenant_id)
        .bind(audit_actor_kind_to_db(&event.actor.kind))
        .bind(&event.actor.actor_id)
        .bind(event.actor.display_name.as_deref())
        .bind(&event.target.resource_type)
        .bind(&event.target.resource_id)
        .bind(&event.target.action_type)
        .bind(audit_event_type_to_db(&event.event_type))
        .bind(json_option(&event.before)?)
        .bind(json_option(&event.after)?)
        .bind(json_option(&event.execution)?)
        .bind(operation_id))
}

fn enqueue_outbox_query<'a>(
    tenant_id: &'a str,
    stream: &'a str,
    aggregate_id: &'a str,
    payload: &'a Value,
    next_attempt_at_ms: u64,
) -> sqlx::query::Query<'a, Postgres, sqlx::postgres::PgArguments> {
    sqlx::query(ENQUEUE_AUDIT_OUTBOX)
        .bind(tenant_id)
        .bind(stream)
        .bind(aggregate_id)
        .bind(payload)
        .bind(next_attempt_at_ms as i64)
}

pub(super) fn validate_audit_outbox_payload(payload: &Value) -> PgRepositoryResult<()> {
    crate::storage::postgres::validate_audit_outbox_payload(payload)
        .map_err(|_| PostgresRepositoryError::UnsafeAuditOutboxPayload)
}

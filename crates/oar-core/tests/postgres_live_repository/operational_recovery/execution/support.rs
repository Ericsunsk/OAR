use super::*;

pub(super) async fn token_grant_updated_at_ms(
    pool: &PgPool,
    tenant_id: &str,
    grant_id: &str,
) -> Result<u64, sqlx::Error> {
    let value = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT floor(extract(epoch from updated_at) * 1000)::bigint
        FROM token_grants
        WHERE tenant_id = $1
          AND id = $2
        "#,
    )
    .bind(tenant_id)
    .bind(grant_id)
    .fetch_one(pool)
    .await?;
    Ok(value as u64)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AuditOutboxRowState {
    pub(super) status: String,
    pub(super) attempt_count: i32,
    pub(super) next_attempt_at_ms: Option<i64>,
    pub(super) sent_at_ms: Option<i64>,
}

pub(super) async fn audit_outbox_row_state(
    pool: &PgPool,
    tenant_id: &str,
    outbox_id: i64,
) -> Result<AuditOutboxRowState, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT
            status,
            attempt_count,
            floor(extract(epoch from next_attempt_at) * 1000)::bigint AS next_attempt_at_ms,
            floor(extract(epoch from sent_at) * 1000)::bigint AS sent_at_ms
        FROM audit_outbox
        WHERE tenant_id = $1
          AND id = $2
        "#,
    )
    .bind(tenant_id)
    .bind(outbox_id)
    .fetch_one(pool)
    .await?;

    Ok(AuditOutboxRowState {
        status: row.try_get("status")?,
        attempt_count: row.try_get("attempt_count")?,
        next_attempt_at_ms: row.try_get("next_attempt_at_ms")?,
        sent_at_ms: row.try_get("sent_at_ms")?,
    })
}

pub(super) async fn audit_event_operation_count(
    pool: &PgPool,
    tenant_id: &str,
    trace_id: &str,
    operation_id: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM audit_events
        WHERE tenant_id = $1
          AND trace_id = $2
          AND operation_id = $3
        "#,
    )
    .bind(tenant_id)
    .bind(trace_id)
    .bind(operation_id)
    .fetch_one(pool)
    .await
}

pub(super) async fn audit_outbox_count_for_trace(
    pool: &PgPool,
    tenant_id: &str,
    trace_id: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM audit_outbox
        WHERE tenant_id = $1
          AND aggregate_id = $2
        "#,
    )
    .bind(tenant_id)
    .bind(trace_id)
    .fetch_one(pool)
    .await
}

use super::*;

pub(in crate::storage::postgres::repository_sqlx) fn audit_event_from_row(
    row: &PgRow,
) -> PgRepositoryResult<AuditEvent> {
    let sequence = non_negative_i64_to_u64(row.try_get("sequence")?, "sequence")?;
    let occurred_at_ms = non_negative_i64_to_u64(row.try_get("occurred_at_ms")?, "occurred_at_ms")?;
    let actor_kind: String = row.try_get("actor_kind")?;
    let event_type: String = row.try_get("event_type")?;

    Ok(AuditEvent {
        event_id: row.try_get("event_id")?,
        trace_id: row.try_get("trace_id")?,
        sequence,
        occurred_at_ms,
        event_type: audit_event_type_from_db(&event_type)?,
        actor: AuditActor {
            kind: audit_actor_kind_from_db(&actor_kind)?,
            actor_id: row.try_get("actor_id")?,
            display_name: row.try_get("actor_display_name")?,
        },
        scope: AuditScope {
            tenant_id: row.try_get("tenant_id")?,
            workspace_id: None,
        },
        target: AuditTarget {
            resource_type: row.try_get("target_resource_type")?,
            resource_id: row.try_get("target_resource_id")?,
            action_type: row.try_get("target_action_type")?,
        },
        before: json_value_option(row.try_get("before_summary")?)?,
        after: json_value_option(row.try_get("after_summary")?)?,
        execution: json_value_option(row.try_get("execution_result")?)?,
    })
}

pub(in crate::storage::postgres::repository_sqlx) fn audit_outbox_message_from_row(
    row: &PgRow,
) -> PgRepositoryResult<AuditOutboxMessage> {
    Ok(AuditOutboxMessage {
        id: row.try_get("id")?,
        tenant_id: row.try_get("tenant_id")?,
        stream: row.try_get("stream")?,
        aggregate_id: row.try_get("aggregate_id")?,
        payload: row.try_get("payload")?,
        attempt_count: row.try_get("attempt_count")?,
        next_attempt_at_ms: row.try_get("next_attempt_at_ms")?,
    })
}

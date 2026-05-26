use crate::action::audit_event::{
    AuditActor, AuditActorKind, AuditEvent, AuditEventType, AuditScope, AuditTarget,
};
use crate::action::confirmed_action::{ActionStatus, ConfirmedAction};
use crate::action::operation_ledger::{LedgerError, OperationRecord, SubmitResult};
use crate::storage::postgres::audit_sql::{
    APPEND_AUDIT_EVENT, ENQUEUE_AUDIT_OUTBOX, FIND_AUDIT_EVENTS_BY_TRACE_ID,
};
use crate::storage::postgres::operation_ledger_sql::{
    GET_BY_IDEMPOTENCY_KEY, MARK_EXECUTING, MARK_FAILED, MARK_SUCCEEDED,
    SUBMIT_CONFIRMED_ACTION_AND_LEDGER,
};
use serde_json::Value;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PostgresRepositoryError {
    #[error("postgres query failed: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("unknown action status from database: {0}")]
    UnknownActionStatus(String),
    #[error("unknown audit actor kind from database: {0}")]
    UnknownAuditActorKind(String),
    #[error("unknown audit event type from database: {0}")]
    UnknownAuditEventType(String),
    #[error("unknown execution status from database: {0}")]
    UnknownExecutionStatus(String),
    #[error("action must be confirmed before persistence: {0:?}")]
    ActionNotConfirmed(ActionStatus),
    #[error("invalid signed integer for {field}: {value}")]
    NegativeInteger { field: &'static str, value: i64 },
    #[error("invalid audit JSON payload: {0}")]
    Json(#[from] serde_json::Error),
}

pub type PgRepositoryResult<T> = Result<T, PostgresRepositoryError>;

#[derive(Debug, Clone)]
pub struct PostgresOperationLedgerRepository {
    pool: PgPool,
}

impl PostgresOperationLedgerRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn submit_confirmed_action(
        &self,
        action: &ConfirmedAction,
        confirmed_at_ms: u64,
        operation_id: &str,
    ) -> PgRepositoryResult<SubmitResult> {
        if action.status != ActionStatus::Confirmed {
            return Err(PostgresRepositoryError::ActionNotConfirmed(action.status));
        }

        let row = sqlx::query(SUBMIT_CONFIRMED_ACTION_AND_LEDGER)
            .bind(&action.action_id)
            .bind(&action.tenant_id)
            .bind(&action.actor_user_id)
            .bind(&action.idempotency_key)
            .bind(confirmed_at_ms as i64)
            .bind(operation_id)
            .fetch_one(&self.pool)
            .await?;
        let created: bool = row.try_get("created")?;
        let record = operation_record_from_row(&row)?;

        if created {
            Ok(SubmitResult::Created(record))
        } else {
            Ok(SubmitResult::Existing(record))
        }
    }

    pub async fn mark_executing(
        &self,
        tenant_id: &str,
        idempotency_key: &str,
        now_ms: u64,
    ) -> Result<OperationRecord, LedgerError> {
        self.transition(MARK_EXECUTING, tenant_id, idempotency_key, None, now_ms)
            .await
    }

    pub async fn mark_succeeded(
        &self,
        tenant_id: &str,
        idempotency_key: &str,
        now_ms: u64,
    ) -> Result<OperationRecord, LedgerError> {
        self.transition(MARK_SUCCEEDED, tenant_id, idempotency_key, None, now_ms)
            .await
    }

    pub async fn mark_failed(
        &self,
        tenant_id: &str,
        idempotency_key: &str,
        error: &str,
        now_ms: u64,
    ) -> Result<OperationRecord, LedgerError> {
        self.transition(MARK_FAILED, tenant_id, idempotency_key, Some(error), now_ms)
            .await
    }

    pub async fn get_by_idempotency_key(
        &self,
        tenant_id: &str,
        idempotency_key: &str,
    ) -> PgRepositoryResult<Option<OperationRecord>> {
        let row = sqlx::query(GET_BY_IDEMPOTENCY_KEY)
            .bind(tenant_id)
            .bind(idempotency_key)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(operation_record_from_row).transpose()
    }

    async fn transition(
        &self,
        sql: &'static str,
        tenant_id: &str,
        idempotency_key: &str,
        error: Option<&str>,
        now_ms: u64,
    ) -> Result<OperationRecord, LedgerError> {
        let target_status = match sql {
            MARK_EXECUTING => ActionStatus::Executing,
            MARK_SUCCEEDED => ActionStatus::Succeeded,
            MARK_FAILED => ActionStatus::Failed,
            _ => ActionStatus::Failed,
        };
        let row = match error {
            Some(error) => {
                sqlx::query(sql)
                    .bind(tenant_id)
                    .bind(idempotency_key)
                    .bind(error)
                    .bind(now_ms as i64)
                    .fetch_optional(&self.pool)
                    .await
            }
            None => {
                sqlx::query(sql)
                    .bind(tenant_id)
                    .bind(idempotency_key)
                    .bind(now_ms as i64)
                    .fetch_optional(&self.pool)
                    .await
            }
        }
        .map_err(|error| LedgerError::UnknownIdempotencyKey(error.to_string()))?;

        if let Some(row) = row {
            return operation_record_from_row(&row)
                .map_err(|error| LedgerError::UnknownIdempotencyKey(error.to_string()));
        }

        match self
            .get_by_idempotency_key(tenant_id, idempotency_key)
            .await
        {
            Ok(Some(record)) if record.status == target_status => Ok(record),
            Ok(Some(record)) => Err(LedgerError::InvalidTransition {
                from: record.status,
                to: target_status,
            }),
            Ok(None) => Err(LedgerError::UnknownIdempotencyKey(
                idempotency_key.to_string(),
            )),
            Err(error) => Err(LedgerError::UnknownIdempotencyKey(error.to_string())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PostgresAuditEventRepository {
    pool: PgPool,
}

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
        sqlx::query(APPEND_AUDIT_EVENT)
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
            .bind(operation_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn find_by_trace_id(&self, trace_id: &str) -> PgRepositoryResult<Vec<AuditEvent>> {
        let rows = sqlx::query(FIND_AUDIT_EVENTS_BY_TRACE_ID)
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
        let row = sqlx::query(ENQUEUE_AUDIT_OUTBOX)
            .bind(tenant_id)
            .bind(stream)
            .bind(aggregate_id)
            .bind(payload)
            .bind(next_attempt_at_ms as i64)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.try_get("id")?)
    }
}

fn operation_record_from_row(row: &PgRow) -> PgRepositoryResult<OperationRecord> {
    let status: String = row.try_get("status")?;
    Ok(OperationRecord {
        operation_id: row.try_get("operation_id")?,
        action_id: row.try_get("action_id")?,
        idempotency_key: row.try_get("idempotency_key")?,
        status: action_status_from_db(&status)?,
        last_error: row.try_get("last_error")?,
    })
}

fn action_status_from_db(value: &str) -> PgRepositoryResult<ActionStatus> {
    match value {
        "proposed" => Ok(ActionStatus::Proposed),
        "confirmed" => Ok(ActionStatus::Confirmed),
        "executing" => Ok(ActionStatus::Executing),
        "succeeded" => Ok(ActionStatus::Succeeded),
        "failed" => Ok(ActionStatus::Failed),
        "cancelled" => Ok(ActionStatus::Cancelled),
        other => Err(PostgresRepositoryError::UnknownActionStatus(
            other.to_string(),
        )),
    }
}

fn audit_event_from_row(row: &PgRow) -> PgRepositoryResult<AuditEvent> {
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

fn json_option<T: serde::Serialize>(value: &Option<T>) -> PgRepositoryResult<Option<Value>> {
    value
        .as_ref()
        .map(serde_json::to_value)
        .transpose()
        .map_err(PostgresRepositoryError::from)
}

fn json_value_option<T>(value: Option<Value>) -> PgRepositoryResult<Option<T>>
where
    T: serde::de::DeserializeOwned,
{
    value
        .map(serde_json::from_value)
        .transpose()
        .map_err(PostgresRepositoryError::from)
}

fn audit_actor_kind_to_db(kind: &AuditActorKind) -> &'static str {
    match kind {
        AuditActorKind::User => "user",
        AuditActorKind::Bot => "bot",
        AuditActorKind::App => "app",
        AuditActorKind::System => "system",
        AuditActorKind::Service => "service",
    }
}

fn audit_actor_kind_from_db(value: &str) -> PgRepositoryResult<AuditActorKind> {
    match value {
        "user" => Ok(AuditActorKind::User),
        "bot" => Ok(AuditActorKind::Bot),
        "app" => Ok(AuditActorKind::App),
        "system" => Ok(AuditActorKind::System),
        "service" => Ok(AuditActorKind::Service),
        other => Err(PostgresRepositoryError::UnknownAuditActorKind(
            other.to_string(),
        )),
    }
}

fn audit_event_type_to_db(event_type: &AuditEventType) -> &'static str {
    match event_type {
        AuditEventType::ConfirmedActionRecorded => "confirmed_action_recorded",
        AuditEventType::DryRunExecuted => "dry_run_executed",
        AuditEventType::ExecutionDenied => "execution_denied",
        AuditEventType::ExecutionSucceeded => "execution_succeeded",
        AuditEventType::ExecutionFailed => "execution_failed",
    }
}

fn audit_event_type_from_db(value: &str) -> PgRepositoryResult<AuditEventType> {
    match value {
        "confirmed_action_recorded" => Ok(AuditEventType::ConfirmedActionRecorded),
        "dry_run_executed" => Ok(AuditEventType::DryRunExecuted),
        "execution_denied" => Ok(AuditEventType::ExecutionDenied),
        "execution_succeeded" => Ok(AuditEventType::ExecutionSucceeded),
        "execution_failed" => Ok(AuditEventType::ExecutionFailed),
        other => Err(PostgresRepositoryError::UnknownAuditEventType(
            other.to_string(),
        )),
    }
}

fn non_negative_i64_to_u64(value: i64, field: &'static str) -> PgRepositoryResult<u64> {
    if value < 0 {
        return Err(PostgresRepositoryError::NegativeInteger { field, value });
    }
    Ok(value as u64)
}

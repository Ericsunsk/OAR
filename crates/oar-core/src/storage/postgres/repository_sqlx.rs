use crate::action::audit_event::{
    AuditActor, AuditActorKind, AuditEvent, AuditEventType, AuditScope, AuditTarget,
};
use crate::action::confirmed_action::{ActionStatus, ConfirmedAction};
use crate::action::operation_ledger::{LedgerError, OperationRecord, SubmitResult};
use crate::domain::identity::{ActorKind, ScopeBoundary, TokenGrantState};
use crate::storage::postgres::audit_sql::{
    APPEND_AUDIT_EVENT, CLAIM_AUDIT_OUTBOX, ENQUEUE_AUDIT_OUTBOX, FIND_AUDIT_EVENTS_BY_TRACE_ID,
    MARK_AUDIT_OUTBOX_FAILED, MARK_AUDIT_OUTBOX_FAILED_FOR_ATTEMPT, MARK_AUDIT_OUTBOX_RETRYABLE,
    MARK_AUDIT_OUTBOX_RETRYABLE_FOR_ATTEMPT, MARK_AUDIT_OUTBOX_SENT,
    MARK_AUDIT_OUTBOX_SENT_FOR_ATTEMPT,
};
use crate::storage::postgres::device_session_sql::{
    ADVANCE_DEVICE_SESSION_CURSOR_CAS, EXPIRE_DEVICE_SESSION, GET_DEVICE_SESSION_BY_ID,
    REVOKE_DEVICE_SESSION, UPSERT_DEVICE_SESSION,
};
use crate::storage::postgres::operation_ledger_sql::{
    GET_BY_IDEMPOTENCY_KEY, MARK_EXECUTING, MARK_FAILED, MARK_SUCCEEDED,
    SUBMIT_CONFIRMED_ACTION_AND_LEDGER,
};
use crate::storage::postgres::token_grant_sql::{
    GET_TOKEN_GRANT_BY_ID, MARK_TOKEN_GRANT_REAUTH_REQUIRED, MARK_TOKEN_GRANT_REFRESH_FAILED,
    REVOKE_TOKEN_GRANT, ROTATE_TOKEN_GRANT, UPSERT_TOKEN_GRANT,
};
use serde_json::Value;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
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
    #[error("unknown device entry point from database: {0}")]
    UnknownDeviceEntryPoint(String),
    #[error("unknown device session state from database: {0}")]
    UnknownDeviceSessionState(String),
    #[error("unknown token grant state from database: {0}")]
    UnknownTokenGrantState(String),
    #[error("unknown identity actor kind from database: {0}")]
    UnknownIdentityActorKind(String),
    #[error("unknown scope boundary from database: {0}")]
    UnknownScopeBoundary(String),
    #[error("action must be confirmed before persistence: {0:?}")]
    ActionNotConfirmed(ActionStatus),
    #[error("tenant mismatch for {field}: expected {expected}, got {actual}")]
    TenantMismatch {
        field: &'static str,
        expected: String,
        actual: String,
    },
    #[error("invalid signed integer for {field}: {value}")]
    NegativeInteger { field: &'static str, value: i64 },
    #[error("invalid audit JSON payload: {0}")]
    Json(#[from] serde_json::Error),
}

pub type PgRepositoryResult<T> = Result<T, PostgresRepositoryError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptedTokenGrantRecord {
    pub id: String,
    pub tenant_id: String,
    pub identity_id: String,
    pub actor_kind: ActorKind,
    pub scope_boundary: ScopeBoundary,
    pub scopes: Vec<String>,
    pub state: TokenGrantState,
    pub issued_at_ms: u64,
    pub expires_at_ms: Option<u64>,
    pub refreshed_at_ms: Option<u64>,
    pub revoked_at_ms: Option<u64>,
    pub reauth_required_at_ms: Option<u64>,
    pub last_refresh_error: Option<String>,
    pub encrypted_oauth_grant: Vec<u8>,
    pub oauth_grant_key_id: String,
    pub oauth_grant_fingerprint: String,
    pub revocation_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredDeviceSession {
    pub id: String,
    pub tenant_id: String,
    pub user_id: String,
    pub entry_point: crate::domain::device_sync::DeviceEntryPoint,
    pub state: crate::domain::device_sync::SessionState,
    pub sync_stream: String,
    pub sync_cursor_value: u64,
    pub sync_cursor_updated_at: SystemTime,
    pub session_identity_hash: String,
    pub last_seen_at: SystemTime,
    pub revoked_at: Option<SystemTime>,
    pub expired_at: Option<SystemTime>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuditOutboxMessage {
    pub id: i64,
    pub tenant_id: String,
    pub stream: String,
    pub aggregate_id: String,
    pub payload: Value,
    pub attempt_count: i32,
    pub next_attempt_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuditOutboxEnvelope {
    pub tenant_id: String,
    pub stream: String,
    pub aggregate_id: String,
    pub payload: Value,
    pub next_attempt_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PostgresExecutionUnitOfWorkReport {
    pub operation: OperationRecord,
    pub outbox_id: Option<i64>,
    pub duplicate: bool,
}

#[derive(Debug, Clone)]
pub struct PostgresTokenGrantRepository {
    pool: PgPool,
}

#[derive(Debug, Clone)]
pub struct PostgresDeviceSessionRepository {
    pool: PgPool,
}

impl PostgresDeviceSessionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn upsert_with_identity_hash(
        &self,
        session: &crate::domain::device_sync::DeviceSession,
        session_identity_hash: &str,
    ) -> PgRepositoryResult<StoredDeviceSession> {
        let row = sqlx::query(UPSERT_DEVICE_SESSION)
            .bind(&session.id.0)
            .bind(&session.tenant_id.0)
            .bind(&session.user_id.0)
            .bind(device_entry_point_to_db(&session.entry_point))
            .bind(device_session_state_to_db(&session.state))
            .bind(&session.cursor.stream)
            .bind(session.cursor.value as i64)
            .bind(system_time_to_ms(session.cursor.updated_at)? as i64)
            .bind(session_identity_hash)
            .bind(system_time_to_ms(session.last_seen_at)? as i64)
            .bind(option_system_time_to_i64_ms(session.revoked_at)?)
            .bind(option_system_time_to_i64_ms(session.expired_at)?)
            .fetch_optional(&self.pool)
            .await?;
        if let Some(row) = row.as_ref() {
            return stored_device_session_from_row(row);
        }

        let conflicting_tenant =
            sqlx::query("SELECT tenant_id FROM device_sessions WHERE id = $1 LIMIT 1")
                .bind(&session.id.0)
                .fetch_optional(&self.pool)
                .await?;

        if let Some(conflict_row) = conflicting_tenant {
            let actual_tenant: String = conflict_row.try_get("tenant_id")?;
            return Err(PostgresRepositoryError::TenantMismatch {
                field: "tenant_id",
                expected: session.tenant_id.0.clone(),
                actual: actual_tenant,
            });
        }

        Err(sqlx::Error::RowNotFound.into())
    }

    pub async fn get_by_id(
        &self,
        tenant_id: &str,
        session_id: &str,
    ) -> PgRepositoryResult<Option<StoredDeviceSession>> {
        let row = sqlx::query(GET_DEVICE_SESSION_BY_ID)
            .bind(tenant_id)
            .bind(session_id)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(stored_device_session_from_row).transpose()
    }

    pub async fn advance_cursor_cas(
        &self,
        tenant_id: &str,
        session_id: &str,
        expected_cursor: u64,
        next_cursor: u64,
        now: SystemTime,
    ) -> PgRepositoryResult<Option<StoredDeviceSession>> {
        let now_ms = system_time_to_ms(now)? as i64;
        let row = sqlx::query(ADVANCE_DEVICE_SESSION_CURSOR_CAS)
            .bind(tenant_id)
            .bind(session_id)
            .bind(next_cursor as i64)
            .bind(now_ms)
            .bind(expected_cursor as i64)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(stored_device_session_from_row).transpose()
    }

    pub async fn revoke(
        &self,
        tenant_id: &str,
        session_id: &str,
        now: SystemTime,
    ) -> PgRepositoryResult<Option<StoredDeviceSession>> {
        let row = sqlx::query(REVOKE_DEVICE_SESSION)
            .bind(tenant_id)
            .bind(session_id)
            .bind(system_time_to_ms(now)? as i64)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(stored_device_session_from_row).transpose()
    }

    pub async fn expire(
        &self,
        tenant_id: &str,
        session_id: &str,
        now: SystemTime,
    ) -> PgRepositoryResult<Option<StoredDeviceSession>> {
        let row = sqlx::query(EXPIRE_DEVICE_SESSION)
            .bind(tenant_id)
            .bind(session_id)
            .bind(system_time_to_ms(now)? as i64)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(stored_device_session_from_row).transpose()
    }
}

impl PostgresTokenGrantRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn upsert_encrypted_grant(
        &self,
        grant: &EncryptedTokenGrantRecord,
    ) -> PgRepositoryResult<EncryptedTokenGrantRecord> {
        let row = sqlx::query(UPSERT_TOKEN_GRANT)
            .bind(&grant.id)
            .bind(&grant.tenant_id)
            .bind(&grant.identity_id)
            .bind(actor_kind_to_db(&grant.actor_kind))
            .bind(scope_boundary_to_db(&grant.scope_boundary))
            .bind(&grant.scopes)
            .bind(token_grant_state_to_db(&grant.state))
            .bind(grant.issued_at_ms as i64)
            .bind(option_u64_to_i64(grant.expires_at_ms))
            .bind(option_u64_to_i64(grant.refreshed_at_ms))
            .bind(option_u64_to_i64(grant.revoked_at_ms))
            .bind(option_u64_to_i64(grant.reauth_required_at_ms))
            .bind(&grant.last_refresh_error)
            .bind(&grant.encrypted_oauth_grant)
            .bind(&grant.oauth_grant_key_id)
            .bind(&grant.oauth_grant_fingerprint)
            .bind(&grant.revocation_reason)
            .fetch_one(&self.pool)
            .await?;
        encrypted_token_grant_from_row(&row)
    }

    pub async fn get_by_id(
        &self,
        tenant_id: &str,
        id: &str,
    ) -> PgRepositoryResult<Option<EncryptedTokenGrantRecord>> {
        let row = sqlx::query(GET_TOKEN_GRANT_BY_ID)
            .bind(tenant_id)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(encrypted_token_grant_from_row).transpose()
    }

    pub async fn rotate_encrypted_grant(
        &self,
        tenant_id: &str,
        id: &str,
        expected_fingerprint: &str,
        expires_at_ms: Option<u64>,
        refreshed_at_ms: u64,
        encrypted_oauth_grant: &[u8],
        oauth_grant_key_id: &str,
        oauth_grant_fingerprint: &str,
    ) -> PgRepositoryResult<Option<EncryptedTokenGrantRecord>> {
        let row = sqlx::query(ROTATE_TOKEN_GRANT)
            .bind(tenant_id)
            .bind(id)
            .bind(expected_fingerprint)
            .bind(option_u64_to_i64(expires_at_ms))
            .bind(refreshed_at_ms as i64)
            .bind(encrypted_oauth_grant)
            .bind(oauth_grant_key_id)
            .bind(oauth_grant_fingerprint)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(encrypted_token_grant_from_row).transpose()
    }

    pub async fn mark_refresh_failed(
        &self,
        tenant_id: &str,
        id: &str,
        expected_fingerprint: &str,
        refreshed_at_ms: u64,
        reason: &str,
    ) -> PgRepositoryResult<Option<EncryptedTokenGrantRecord>> {
        let row = sqlx::query(MARK_TOKEN_GRANT_REFRESH_FAILED)
            .bind(tenant_id)
            .bind(id)
            .bind(expected_fingerprint)
            .bind(refreshed_at_ms as i64)
            .bind(reason)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(encrypted_token_grant_from_row).transpose()
    }

    pub async fn mark_reauth_required(
        &self,
        tenant_id: &str,
        id: &str,
        expected_fingerprint: &str,
        reauth_required_at_ms: u64,
        reason: &str,
    ) -> PgRepositoryResult<Option<EncryptedTokenGrantRecord>> {
        let row = sqlx::query(MARK_TOKEN_GRANT_REAUTH_REQUIRED)
            .bind(tenant_id)
            .bind(id)
            .bind(expected_fingerprint)
            .bind(reauth_required_at_ms as i64)
            .bind(reason)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(encrypted_token_grant_from_row).transpose()
    }

    pub async fn revoke(
        &self,
        tenant_id: &str,
        id: &str,
        revoked_at_ms: u64,
        reason: &str,
    ) -> PgRepositoryResult<Option<EncryptedTokenGrantRecord>> {
        let row = sqlx::query(REVOKE_TOKEN_GRANT)
            .bind(tenant_id)
            .bind(id)
            .bind(revoked_at_ms as i64)
            .bind(reason)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(encrypted_token_grant_from_row).transpose()
    }
}

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

        submit_confirmed_action_with_executor(&self.pool, action, confirmed_at_ms, operation_id)
            .await
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
pub struct PostgresExecutionUnitOfWork {
    pool: PgPool,
}

impl PostgresExecutionUnitOfWork {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn record_confirmation(
        &self,
        action: &ConfirmedAction,
        confirmed_at_ms: u64,
        operation_id: &str,
        event: &AuditEvent,
        outbox: &AuditOutboxEnvelope,
    ) -> PgRepositoryResult<PostgresExecutionUnitOfWorkReport> {
        validate_uow_tenant(&action.tenant_id, event, outbox)?;

        let mut tx = self.pool.begin().await?;
        let submit =
            submit_confirmed_action_in_tx(&mut tx, action, confirmed_at_ms, operation_id).await?;
        let (operation, duplicate) = submit_result_parts(submit);

        let outbox_id = if duplicate {
            None
        } else {
            append_audit_event_in_tx(&mut tx, event, Some(&operation.operation_id)).await?;
            Some(enqueue_outbox_in_tx(&mut tx, outbox).await?)
        };
        tx.commit().await?;

        Ok(PostgresExecutionUnitOfWorkReport {
            operation,
            outbox_id,
            duplicate,
        })
    }

    pub async fn record_dry_run(
        &self,
        tenant_id: &str,
        idempotency_key: &str,
        now_ms: u64,
        event: &AuditEvent,
        outbox: &AuditOutboxEnvelope,
    ) -> PgRepositoryResult<PostgresExecutionUnitOfWorkReport> {
        self.record_status_transition(
            MARK_EXECUTING,
            ActionStatus::Executing,
            tenant_id,
            idempotency_key,
            None,
            now_ms,
            event,
            outbox,
        )
        .await
    }

    pub async fn record_success(
        &self,
        tenant_id: &str,
        idempotency_key: &str,
        now_ms: u64,
        event: &AuditEvent,
        outbox: &AuditOutboxEnvelope,
    ) -> PgRepositoryResult<PostgresExecutionUnitOfWorkReport> {
        self.record_status_transition(
            MARK_SUCCEEDED,
            ActionStatus::Succeeded,
            tenant_id,
            idempotency_key,
            None,
            now_ms,
            event,
            outbox,
        )
        .await
    }

    pub async fn record_failure(
        &self,
        tenant_id: &str,
        idempotency_key: &str,
        error: &str,
        now_ms: u64,
        event: &AuditEvent,
        outbox: &AuditOutboxEnvelope,
    ) -> PgRepositoryResult<PostgresExecutionUnitOfWorkReport> {
        self.record_status_transition(
            MARK_FAILED,
            ActionStatus::Failed,
            tenant_id,
            idempotency_key,
            Some(error),
            now_ms,
            event,
            outbox,
        )
        .await
    }

    async fn record_status_transition(
        &self,
        sql: &'static str,
        target_status: ActionStatus,
        tenant_id: &str,
        idempotency_key: &str,
        error: Option<&str>,
        now_ms: u64,
        event: &AuditEvent,
        outbox: &AuditOutboxEnvelope,
    ) -> PgRepositoryResult<PostgresExecutionUnitOfWorkReport> {
        validate_uow_tenant(tenant_id, event, outbox)?;

        let mut tx = self.pool.begin().await?;
        let (operation, duplicate) = transition_in_tx(
            &mut tx,
            sql,
            target_status,
            tenant_id,
            idempotency_key,
            error,
            now_ms,
        )
        .await?;

        let outbox_id = if duplicate {
            None
        } else {
            append_audit_event_in_tx(&mut tx, event, Some(&operation.operation_id)).await?;
            Some(enqueue_outbox_in_tx(&mut tx, outbox).await?)
        };
        tx.commit().await?;

        Ok(PostgresExecutionUnitOfWorkReport {
            operation,
            outbox_id,
            duplicate,
        })
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

async fn submit_confirmed_action_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    action: &ConfirmedAction,
    confirmed_at_ms: u64,
    operation_id: &str,
) -> PgRepositoryResult<SubmitResult> {
    if action.status != ActionStatus::Confirmed {
        return Err(PostgresRepositoryError::ActionNotConfirmed(action.status));
    }

    submit_confirmed_action_with_executor(&mut **tx, action, confirmed_at_ms, operation_id).await
}

async fn submit_confirmed_action_with_executor<'e, E>(
    executor: E,
    action: &ConfirmedAction,
    confirmed_at_ms: u64,
    operation_id: &str,
) -> PgRepositoryResult<SubmitResult>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let row = sqlx::query(SUBMIT_CONFIRMED_ACTION_AND_LEDGER)
        .bind(&action.action_id)
        .bind(&action.tenant_id)
        .bind(&action.actor_user_id)
        .bind(&action.idempotency_key)
        .bind(confirmed_at_ms as i64)
        .bind(operation_id)
        .fetch_one(executor)
        .await?;
    let created: bool = row.try_get("created")?;
    let record = operation_record_from_row(&row)?;

    if created {
        Ok(SubmitResult::Created(record))
    } else {
        Ok(SubmitResult::Existing(record))
    }
}

async fn transition_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    sql: &'static str,
    target_status: ActionStatus,
    tenant_id: &str,
    idempotency_key: &str,
    error: Option<&str>,
    now_ms: u64,
) -> PgRepositoryResult<(OperationRecord, bool)> {
    let row = match error {
        Some(error) => {
            sqlx::query(sql)
                .bind(tenant_id)
                .bind(idempotency_key)
                .bind(error)
                .bind(now_ms as i64)
                .fetch_optional(&mut **tx)
                .await?
        }
        None => {
            sqlx::query(sql)
                .bind(tenant_id)
                .bind(idempotency_key)
                .bind(now_ms as i64)
                .fetch_optional(&mut **tx)
                .await?
        }
    };

    if let Some(row) = row {
        return Ok((operation_record_from_row(&row)?, false));
    }

    let existing = sqlx::query(GET_BY_IDEMPOTENCY_KEY)
        .bind(tenant_id)
        .bind(idempotency_key)
        .fetch_optional(&mut **tx)
        .await?;

    match existing {
        Some(row) => {
            let record = operation_record_from_row(&row)?;
            if record.status == target_status {
                Ok((record, true))
            } else {
                Err(sqlx::Error::RowNotFound.into())
            }
        }
        None => Err(sqlx::Error::RowNotFound.into()),
    }
}

fn validate_uow_tenant(
    expected_tenant_id: &str,
    event: &AuditEvent,
    outbox: &AuditOutboxEnvelope,
) -> PgRepositoryResult<()> {
    if event.scope.tenant_id != expected_tenant_id {
        return Err(PostgresRepositoryError::TenantMismatch {
            field: "event.scope.tenant_id",
            expected: expected_tenant_id.to_string(),
            actual: event.scope.tenant_id.clone(),
        });
    }

    if outbox.tenant_id != expected_tenant_id {
        return Err(PostgresRepositoryError::TenantMismatch {
            field: "outbox.tenant_id",
            expected: expected_tenant_id.to_string(),
            actual: outbox.tenant_id.clone(),
        });
    }

    Ok(())
}

async fn append_audit_event_in_tx(
    tx: &mut Transaction<'_, Postgres>,
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
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn enqueue_outbox_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    outbox: &AuditOutboxEnvelope,
) -> PgRepositoryResult<i64> {
    let row = sqlx::query(ENQUEUE_AUDIT_OUTBOX)
        .bind(&outbox.tenant_id)
        .bind(&outbox.stream)
        .bind(&outbox.aggregate_id)
        .bind(&outbox.payload)
        .bind(outbox.next_attempt_at_ms as i64)
        .fetch_one(&mut **tx)
        .await?;
    Ok(row.try_get("id")?)
}

fn submit_result_parts(result: SubmitResult) -> (OperationRecord, bool) {
    match result {
        SubmitResult::Created(record) => (record, false),
        SubmitResult::Existing(record) => (record, true),
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

fn audit_outbox_message_from_row(row: &PgRow) -> PgRepositoryResult<AuditOutboxMessage> {
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

fn encrypted_token_grant_from_row(row: &PgRow) -> PgRepositoryResult<EncryptedTokenGrantRecord> {
    let actor_kind: String = row.try_get("actor_kind")?;
    let scope_boundary: String = row.try_get("scope_boundary")?;
    let state: String = row.try_get("state")?;

    Ok(EncryptedTokenGrantRecord {
        id: row.try_get("id")?,
        tenant_id: row.try_get("tenant_id")?,
        identity_id: row.try_get("identity_id")?,
        actor_kind: identity_actor_kind_from_db(&actor_kind)?,
        scope_boundary: scope_boundary_from_db(&scope_boundary)?,
        scopes: row.try_get("scopes")?,
        state: token_grant_state_from_db(&state)?,
        issued_at_ms: non_negative_i64_to_u64(row.try_get("issued_at_ms")?, "issued_at_ms")?,
        expires_at_ms: optional_non_negative_i64_to_u64(
            row.try_get("expires_at_ms")?,
            "expires_at_ms",
        )?,
        refreshed_at_ms: optional_non_negative_i64_to_u64(
            row.try_get("refreshed_at_ms")?,
            "refreshed_at_ms",
        )?,
        revoked_at_ms: optional_non_negative_i64_to_u64(
            row.try_get("revoked_at_ms")?,
            "revoked_at_ms",
        )?,
        reauth_required_at_ms: optional_non_negative_i64_to_u64(
            row.try_get("reauth_required_at_ms")?,
            "reauth_required_at_ms",
        )?,
        last_refresh_error: row.try_get("last_refresh_error")?,
        encrypted_oauth_grant: row.try_get("encrypted_oauth_grant")?,
        oauth_grant_key_id: row.try_get("oauth_grant_key_id")?,
        oauth_grant_fingerprint: row.try_get("oauth_grant_fingerprint")?,
        revocation_reason: row.try_get("revocation_reason")?,
    })
}

fn stored_device_session_from_row(row: &PgRow) -> PgRepositoryResult<StoredDeviceSession> {
    let entry_point: String = row.try_get("entry_point")?;
    let state: String = row.try_get("state")?;

    Ok(StoredDeviceSession {
        id: row.try_get("id")?,
        tenant_id: row.try_get("tenant_id")?,
        user_id: row.try_get("user_id")?,
        entry_point: device_entry_point_from_db(&entry_point)?,
        state: device_session_state_from_db(&state)?,
        sync_stream: row.try_get("sync_stream")?,
        sync_cursor_value: non_negative_i64_to_u64(
            row.try_get("sync_cursor_value")?,
            "sync_cursor_value",
        )?,
        sync_cursor_updated_at: ms_to_system_time(non_negative_i64_to_u64(
            row.try_get("sync_cursor_updated_at_ms")?,
            "sync_cursor_updated_at_ms",
        )?),
        session_identity_hash: row.try_get("session_identity_hash")?,
        last_seen_at: ms_to_system_time(non_negative_i64_to_u64(
            row.try_get("last_seen_at_ms")?,
            "last_seen_at_ms",
        )?),
        revoked_at: optional_non_negative_i64_to_u64(
            row.try_get("revoked_at_ms")?,
            "revoked_at_ms",
        )?
        .map(ms_to_system_time),
        expired_at: optional_non_negative_i64_to_u64(
            row.try_get("expired_at_ms")?,
            "expired_at_ms",
        )?
        .map(ms_to_system_time),
    })
}

fn option_u64_to_i64(value: Option<u64>) -> Option<i64> {
    value.map(|value| value as i64)
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

fn actor_kind_to_db(kind: &ActorKind) -> &'static str {
    match kind {
        ActorKind::User => "user",
        ActorKind::Bot => "bot",
        ActorKind::App => "app",
        ActorKind::Service => "service",
    }
}

fn identity_actor_kind_from_db(value: &str) -> PgRepositoryResult<ActorKind> {
    match value {
        "user" => Ok(ActorKind::User),
        "bot" => Ok(ActorKind::Bot),
        "app" => Ok(ActorKind::App),
        "service" => Ok(ActorKind::Service),
        other => Err(PostgresRepositoryError::UnknownIdentityActorKind(
            other.to_string(),
        )),
    }
}

fn scope_boundary_to_db(boundary: &ScopeBoundary) -> &'static str {
    match boundary {
        ScopeBoundary::Tenant => "tenant",
        ScopeBoundary::User => "user",
        ScopeBoundary::Admin => "admin",
        ScopeBoundary::Bot => "bot",
        ScopeBoundary::Service => "service",
    }
}

fn scope_boundary_from_db(value: &str) -> PgRepositoryResult<ScopeBoundary> {
    match value {
        "tenant" => Ok(ScopeBoundary::Tenant),
        "user" => Ok(ScopeBoundary::User),
        "admin" => Ok(ScopeBoundary::Admin),
        "bot" => Ok(ScopeBoundary::Bot),
        "service" => Ok(ScopeBoundary::Service),
        other => Err(PostgresRepositoryError::UnknownScopeBoundary(
            other.to_string(),
        )),
    }
}

fn token_grant_state_to_db(state: &TokenGrantState) -> &'static str {
    match state {
        TokenGrantState::Valid => "valid",
        TokenGrantState::NeedsRefresh => "needs_refresh",
        TokenGrantState::Expired => "expired",
        TokenGrantState::Revoked => "revoked",
        TokenGrantState::ReauthRequired => "reauth_required",
    }
}

fn token_grant_state_from_db(value: &str) -> PgRepositoryResult<TokenGrantState> {
    match value {
        "valid" => Ok(TokenGrantState::Valid),
        "needs_refresh" => Ok(TokenGrantState::NeedsRefresh),
        "expired" => Ok(TokenGrantState::Expired),
        "revoked" => Ok(TokenGrantState::Revoked),
        "reauth_required" => Ok(TokenGrantState::ReauthRequired),
        other => Err(PostgresRepositoryError::UnknownTokenGrantState(
            other.to_string(),
        )),
    }
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

fn optional_non_negative_i64_to_u64(
    value: Option<i64>,
    field: &'static str,
) -> PgRepositoryResult<Option<u64>> {
    value
        .map(|value| non_negative_i64_to_u64(value, field))
        .transpose()
}

fn device_entry_point_to_db(value: &crate::domain::device_sync::DeviceEntryPoint) -> &'static str {
    match value {
        crate::domain::device_sync::DeviceEntryPoint::MacOs => "macos",
        crate::domain::device_sync::DeviceEntryPoint::Ios => "ios",
        crate::domain::device_sync::DeviceEntryPoint::Web => "web",
        crate::domain::device_sync::DeviceEntryPoint::Lark => "lark",
    }
}

fn device_entry_point_from_db(
    value: &str,
) -> PgRepositoryResult<crate::domain::device_sync::DeviceEntryPoint> {
    match value {
        "macos" => Ok(crate::domain::device_sync::DeviceEntryPoint::MacOs),
        "ios" => Ok(crate::domain::device_sync::DeviceEntryPoint::Ios),
        "web" => Ok(crate::domain::device_sync::DeviceEntryPoint::Web),
        "lark" => Ok(crate::domain::device_sync::DeviceEntryPoint::Lark),
        other => Err(PostgresRepositoryError::UnknownDeviceEntryPoint(
            other.to_string(),
        )),
    }
}

fn device_session_state_to_db(value: &crate::domain::device_sync::SessionState) -> &'static str {
    match value {
        crate::domain::device_sync::SessionState::Active => "active",
        crate::domain::device_sync::SessionState::Revoked => "revoked",
        crate::domain::device_sync::SessionState::Expired => "expired",
    }
}

fn device_session_state_from_db(
    value: &str,
) -> PgRepositoryResult<crate::domain::device_sync::SessionState> {
    match value {
        "active" => Ok(crate::domain::device_sync::SessionState::Active),
        "revoked" => Ok(crate::domain::device_sync::SessionState::Revoked),
        "expired" => Ok(crate::domain::device_sync::SessionState::Expired),
        other => Err(PostgresRepositoryError::UnknownDeviceSessionState(
            other.to_string(),
        )),
    }
}

fn system_time_to_ms(value: SystemTime) -> PgRepositoryResult<u64> {
    value
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .map_err(|_| PostgresRepositoryError::NegativeInteger {
            field: "system_time",
            value: -1,
        })
}

fn option_system_time_to_i64_ms(value: Option<SystemTime>) -> PgRepositoryResult<Option<i64>> {
    value
        .map(system_time_to_ms)
        .transpose()
        .map(|maybe| maybe.map(|ms| ms as i64))
}

fn ms_to_system_time(value: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_millis(value)
}

use crate::action::audit_event::{AuditActor, AuditEvent, AuditScope, AuditTarget};
use crate::action::confirmed_action::{ActionStatus, ConfirmedAction};
use crate::action::operation_ledger::{LedgerError, OperationRecord, SubmitResult};
use crate::action::token_refresh_audit::{token_refresh_audit_event, TokenRefreshAuditContext};
use crate::domain::identity::{
    ActorKind, LarkIdentity, OarUser, OarUserStatus, ScopeBoundary, Tenant, TenantStatus,
    TokenGrantState,
};
use crate::domain::token_refresh::bridge::{plan_token_refresh_command, TokenRefreshBridgeError};
use crate::domain::token_refresh::service::{
    token_refresh_short_circuit_report, AuthRefreshAdapter,
};
use crate::domain::token_refresh::types::{
    TokenRefreshApplyResult, TokenRefreshAuditSummary, TokenRefreshGrantSnapshot,
    TokenRefreshPlannedCommand, TokenRefreshReportStatus, TokenRefreshRepositoryCommand,
    TokenRefreshServiceReport,
};
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
use crate::storage::postgres::identity_sql::{
    GET_LARK_IDENTITY_BY_ACTOR_EXTERNAL, GET_LARK_IDENTITY_BY_ID, GET_OAR_USER_BY_ID,
    GET_TENANT_BY_ID, UPSERT_LARK_IDENTITY, UPSERT_OAR_USER, UPSERT_TENANT,
};
use crate::storage::postgres::operation_ledger_sql::{
    GET_BY_IDEMPOTENCY_KEY, MARK_EXECUTING, MARK_FAILED, MARK_SUCCEEDED,
    SUBMIT_CONFIRMED_ACTION_AND_LEDGER,
};
use crate::storage::postgres::token_grant_sql::{
    GET_TOKEN_GRANT_BY_ID, LIST_TOKEN_REFRESH_CANDIDATE_SNAPSHOTS,
    MARK_TOKEN_GRANT_REAUTH_REQUIRED, MARK_TOKEN_GRANT_REFRESH_FAILED, REVOKE_TOKEN_GRANT,
    ROTATE_TOKEN_GRANT, UPSERT_TOKEN_GRANT,
};
use serde_json::Value;
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::time::SystemTime;
use thiserror::Error;

mod codec;
mod rows;
mod util;

use codec::*;
use rows::*;
use util::*;

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
    #[error("unknown tenant status from database: {0}")]
    UnknownTenantStatus(String),
    #[error("unknown oar user status from database: {0}")]
    UnknownOarUserStatus(String),
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
    #[error("lark identity actor external binding conflict")]
    LarkIdentityActorExternalBindingConflict {
        tenant_id: String,
        actor_kind: ActorKind,
        actor_external_id: String,
    },
    #[error("invalid signed integer for {field}: {value}")]
    NegativeInteger { field: &'static str, value: i64 },
    #[error("invalid audit JSON payload: {0}")]
    Json(#[from] serde_json::Error),
    #[error("token refresh decision bridge failed")]
    TokenRefreshDecisionBridge(#[source] TokenRefreshBridgeError),
    #[error("invalid operation status transition from {from:?} to {to:?}")]
    InvalidOperationStatusTransition {
        from: ActionStatus,
        to: ActionStatus,
    },
    #[error("unknown operation idempotency key: {0}")]
    UnknownOperationIdempotencyKey(String),
    #[error(
        "token refresh planned command mismatch for {field}: expected {expected}, got {actual}"
    )]
    TokenRefreshPlanMismatch {
        field: &'static str,
        expected: String,
        actual: String,
    },
}

pub type PgRepositoryResult<T> = Result<T, PostgresRepositoryError>;

const REDACTED_TENANT_ACTUAL: &str = "<redacted>";
const REDACTED_REFRESH_ERROR: &str = "<redacted refresh error>";
const MAX_REFRESH_ERROR_CHARS: usize = 256;

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
pub struct RotateEncryptedGrantRequest<'a> {
    pub tenant_id: &'a str,
    pub id: &'a str,
    pub expected_fingerprint: &'a str,
    pub expires_at_ms: Option<u64>,
    pub refreshed_at_ms: u64,
    pub encrypted_oauth_grant: &'a [u8],
    pub oauth_grant_key_id: &'a str,
    pub oauth_grant_fingerprint: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredTenant {
    pub id: String,
    pub display_name: String,
    pub status: TenantStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredOarUser {
    pub id: String,
    pub tenant_id: String,
    pub display_name: String,
    pub status: OarUserStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredLarkIdentity {
    pub id: String,
    pub tenant_id: String,
    pub actor_kind: ActorKind,
    pub actor_external_id: String,
    pub display_name: Option<String>,
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

struct StatusTransitionRequest<'a> {
    sql: &'static str,
    target_status: ActionStatus,
    tenant_id: &'a str,
    idempotency_key: &'a str,
    error: Option<&'a str>,
    now_ms: u64,
    event: &'a AuditEvent,
    outbox: &'a AuditOutboxEnvelope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostgresTokenRefreshUnitOfWorkReport {
    pub apply_result: Option<TokenRefreshApplyResult>,
    pub event: AuditEvent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostgresTokenRefreshOrchestratorReport {
    pub service_report: TokenRefreshServiceReport,
    pub event: AuditEvent,
}

#[derive(Clone)]
pub struct PostgresTokenRefreshSweepRequest {
    pub tenant_id: String,
    pub due_before: SystemTime,
    pub limit: u32,
    pub now: SystemTime,
    pub audit_trace_id: String,
    pub audit_sequence_start: u64,
    pub occurred_at_ms: u64,
    pub actor: AuditActor,
    pub workspace_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostgresTokenRefreshSweepReport {
    pub candidate_count: usize,
    pub attempted_count: usize,
    pub reports: Vec<PostgresTokenRefreshOrchestratorReport>,
}

#[derive(Debug, Clone)]
pub struct PostgresTokenGrantRepository {
    pool: PgPool,
}

#[derive(Debug, Clone)]
pub struct PostgresTokenRefreshOrchestrator<A>
where
    A: AuthRefreshAdapter,
{
    adapter: A,
    uow: PostgresTokenRefreshUnitOfWork,
    audit: PostgresAuditEventRepository,
}

#[derive(Debug, Clone)]
pub struct PostgresTokenRefreshSweep<A>
where
    A: AuthRefreshAdapter,
{
    candidates: PostgresTokenGrantRepository,
    orchestrator: PostgresTokenRefreshOrchestrator<A>,
}

#[derive(Debug, Clone)]
pub struct PostgresDeviceSessionRepository {
    pool: PgPool,
}

#[derive(Debug, Clone)]
pub struct PostgresTenantRepository {
    pool: PgPool,
}

#[derive(Debug, Clone)]
pub struct PostgresOarUserRepository {
    pool: PgPool,
}

#[derive(Debug, Clone)]
pub struct PostgresLarkIdentityRepository {
    pool: PgPool,
}

#[derive(Debug, Clone)]
pub struct PostgresIdentityRepository {
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

        let conflicting_tenant = sqlx::query("SELECT 1 FROM device_sessions WHERE id = $1 LIMIT 1")
            .bind(&session.id.0)
            .fetch_optional(&self.pool)
            .await?;

        if conflicting_tenant.is_some() {
            return Err(PostgresRepositoryError::TenantMismatch {
                field: "tenant_id",
                expected: session.tenant_id.0.clone(),
                actual: redacted_tenant_actual(),
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

    pub async fn apply_refresh_command(
        &self,
        command: TokenRefreshRepositoryCommand,
    ) -> PgRepositoryResult<Option<EncryptedTokenGrantRecord>> {
        match command {
            TokenRefreshRepositoryCommand::RotateGrantCas {
                grant_id,
                tenant_id,
                expected_fingerprint,
                expires_at_ms,
                refreshed_at_ms,
                encrypted_grant_blob,
                grant_key_id,
                new_fingerprint,
            } => {
                self.rotate_encrypted_grant(RotateEncryptedGrantRequest {
                    tenant_id: &tenant_id.0,
                    id: &grant_id.0,
                    expected_fingerprint: &expected_fingerprint,
                    expires_at_ms,
                    refreshed_at_ms,
                    encrypted_oauth_grant: &encrypted_grant_blob.0,
                    oauth_grant_key_id: &grant_key_id,
                    oauth_grant_fingerprint: &new_fingerprint,
                })
                .await
            }
            TokenRefreshRepositoryCommand::MarkNeedsRefresh {
                grant_id,
                tenant_id,
                expected_fingerprint,
                refreshed_at_ms,
                safe_error,
            } => {
                self.mark_refresh_failed(
                    &tenant_id.0,
                    &grant_id.0,
                    &expected_fingerprint,
                    refreshed_at_ms,
                    &safe_error,
                )
                .await
            }
            TokenRefreshRepositoryCommand::MarkReauthRequired {
                grant_id,
                tenant_id,
                expected_fingerprint,
                reauth_required_at_ms,
                safe_error,
            } => {
                self.mark_reauth_required(
                    &tenant_id.0,
                    &grant_id.0,
                    &expected_fingerprint,
                    reauth_required_at_ms,
                    &safe_error,
                )
                .await
            }
        }
    }

    pub async fn rotate_encrypted_grant(
        &self,
        request: RotateEncryptedGrantRequest<'_>,
    ) -> PgRepositoryResult<Option<EncryptedTokenGrantRecord>> {
        let row = sqlx::query(ROTATE_TOKEN_GRANT)
            .bind(request.tenant_id)
            .bind(request.id)
            .bind(request.expected_fingerprint)
            .bind(option_u64_to_i64(request.expires_at_ms))
            .bind(request.refreshed_at_ms as i64)
            .bind(request.encrypted_oauth_grant)
            .bind(request.oauth_grant_key_id)
            .bind(request.oauth_grant_fingerprint)
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
        let reason = sanitize_refresh_error_for_storage(reason);
        let row = sqlx::query(MARK_TOKEN_GRANT_REFRESH_FAILED)
            .bind(tenant_id)
            .bind(id)
            .bind(expected_fingerprint)
            .bind(refreshed_at_ms as i64)
            .bind(&reason)
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
        let reason = sanitize_refresh_error_for_storage(reason);
        let row = sqlx::query(MARK_TOKEN_GRANT_REAUTH_REQUIRED)
            .bind(tenant_id)
            .bind(id)
            .bind(expected_fingerprint)
            .bind(reauth_required_at_ms as i64)
            .bind(&reason)
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

    pub async fn list_refresh_candidate_snapshots(
        &self,
        tenant_id: &str,
        due_before: SystemTime,
        limit: u32,
    ) -> PgRepositoryResult<Vec<TokenRefreshGrantSnapshot>> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let due_before_ms = system_time_to_ms(due_before)? as i64;
        let rows = sqlx::query(LIST_TOKEN_REFRESH_CANDIDATE_SNAPSHOTS)
            .bind(tenant_id)
            .bind(due_before_ms)
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await?;

        rows.iter().map(token_refresh_snapshot_from_row).collect()
    }
}

impl PostgresTenantRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn upsert(&self, tenant: &Tenant) -> PgRepositoryResult<StoredTenant> {
        let row = sqlx::query(UPSERT_TENANT)
            .bind(&tenant.id.0)
            .bind(&tenant.display_name)
            .bind(tenant_status_to_db(&tenant.status))
            .fetch_one(&self.pool)
            .await?;
        stored_tenant_from_row(&row)
    }

    pub async fn get_by_id(&self, tenant_id: &str) -> PgRepositoryResult<Option<StoredTenant>> {
        let row = sqlx::query(GET_TENANT_BY_ID)
            .bind(tenant_id)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(stored_tenant_from_row).transpose()
    }
}

impl PostgresOarUserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn upsert(&self, user: &OarUser) -> PgRepositoryResult<StoredOarUser> {
        let row = sqlx::query(UPSERT_OAR_USER)
            .bind(&user.id.0)
            .bind(&user.tenant_id.0)
            .bind(&user.display_name)
            .bind(oar_user_status_to_db(&user.status))
            .fetch_optional(&self.pool)
            .await?;
        if let Some(row) = row.as_ref() {
            return stored_oar_user_from_row(row);
        }

        let conflicting_tenant = sqlx::query("SELECT 1 FROM oar_users WHERE id = $1 LIMIT 1")
            .bind(&user.id.0)
            .fetch_optional(&self.pool)
            .await?;

        if conflicting_tenant.is_some() {
            return Err(PostgresRepositoryError::TenantMismatch {
                field: "tenant_id",
                expected: user.tenant_id.0.clone(),
                actual: redacted_tenant_actual(),
            });
        }

        Err(sqlx::Error::RowNotFound.into())
    }

    pub async fn get_by_id(
        &self,
        tenant_id: &str,
        user_id: &str,
    ) -> PgRepositoryResult<Option<StoredOarUser>> {
        let row = sqlx::query(GET_OAR_USER_BY_ID)
            .bind(tenant_id)
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(stored_oar_user_from_row).transpose()
    }
}

impl PostgresLarkIdentityRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn upsert(&self, identity: &LarkIdentity) -> PgRepositoryResult<StoredLarkIdentity> {
        let row = match sqlx::query(UPSERT_LARK_IDENTITY)
            .bind(&identity.id.0)
            .bind(&identity.tenant_id.0)
            .bind(actor_kind_to_db(&identity.actor_kind))
            .bind(&identity.actor_external_id)
            .bind(&identity.display_name)
            .fetch_optional(&self.pool)
            .await
        {
            Ok(row) => row,
            Err(error) if is_unique_violation(&error) => {
                let conflicting_row = sqlx::query(GET_LARK_IDENTITY_BY_ACTOR_EXTERNAL)
                    .bind(&identity.tenant_id.0)
                    .bind(actor_kind_to_db(&identity.actor_kind))
                    .bind(&identity.actor_external_id)
                    .fetch_optional(&self.pool)
                    .await?;

                if let Some(conflicting_row) = conflicting_row.as_ref() {
                    let conflicting = stored_lark_identity_from_row(conflicting_row)?;
                    if conflicting.id != identity.id.0 {
                        return Err(
                            PostgresRepositoryError::LarkIdentityActorExternalBindingConflict {
                                tenant_id: identity.tenant_id.0.clone(),
                                actor_kind: identity.actor_kind,
                                actor_external_id: identity.actor_external_id.clone(),
                            },
                        );
                    }
                }

                return Err(error.into());
            }
            Err(error) => return Err(error.into()),
        };
        if let Some(row) = row.as_ref() {
            return stored_lark_identity_from_row(row);
        }

        let conflicting_tenant = sqlx::query("SELECT 1 FROM lark_identities WHERE id = $1 LIMIT 1")
            .bind(&identity.id.0)
            .fetch_optional(&self.pool)
            .await?;

        if conflicting_tenant.is_some() {
            return Err(PostgresRepositoryError::TenantMismatch {
                field: "tenant_id",
                expected: identity.tenant_id.0.clone(),
                actual: redacted_tenant_actual(),
            });
        }

        Err(sqlx::Error::RowNotFound.into())
    }

    pub async fn get_by_id(
        &self,
        tenant_id: &str,
        identity_id: &str,
    ) -> PgRepositoryResult<Option<StoredLarkIdentity>> {
        let row = sqlx::query(GET_LARK_IDENTITY_BY_ID)
            .bind(tenant_id)
            .bind(identity_id)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(stored_lark_identity_from_row).transpose()
    }

    pub async fn get_by_actor_external_id(
        &self,
        tenant_id: &str,
        actor_kind: ActorKind,
        actor_external_id: &str,
    ) -> PgRepositoryResult<Option<StoredLarkIdentity>> {
        let row = sqlx::query(GET_LARK_IDENTITY_BY_ACTOR_EXTERNAL)
            .bind(tenant_id)
            .bind(actor_kind_to_db(&actor_kind))
            .bind(actor_external_id)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(stored_lark_identity_from_row).transpose()
    }
}

impl PostgresIdentityRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub fn tenants(&self) -> PostgresTenantRepository {
        PostgresTenantRepository::new(self.pool.clone())
    }

    pub fn users(&self) -> PostgresOarUserRepository {
        PostgresOarUserRepository::new(self.pool.clone())
    }

    pub fn identities(&self) -> PostgresLarkIdentityRepository {
        PostgresLarkIdentityRepository::new(self.pool.clone())
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

#[derive(Debug, Clone)]
pub struct PostgresTokenRefreshUnitOfWork {
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
        self.record_status_transition(StatusTransitionRequest {
            sql: MARK_EXECUTING,
            target_status: ActionStatus::Executing,
            tenant_id,
            idempotency_key,
            error: None,
            now_ms,
            event,
            outbox,
        })
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
        self.record_status_transition(StatusTransitionRequest {
            sql: MARK_SUCCEEDED,
            target_status: ActionStatus::Succeeded,
            tenant_id,
            idempotency_key,
            error: None,
            now_ms,
            event,
            outbox,
        })
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
        self.record_status_transition(StatusTransitionRequest {
            sql: MARK_FAILED,
            target_status: ActionStatus::Failed,
            tenant_id,
            idempotency_key,
            error: Some(error),
            now_ms,
            event,
            outbox,
        })
        .await
    }

    async fn record_status_transition(
        &self,
        request: StatusTransitionRequest<'_>,
    ) -> PgRepositoryResult<PostgresExecutionUnitOfWorkReport> {
        validate_uow_tenant(request.tenant_id, request.event, request.outbox)?;

        let mut tx = self.pool.begin().await?;
        let (operation, duplicate) = transition_in_tx(
            &mut tx,
            request.sql,
            request.target_status,
            request.tenant_id,
            request.idempotency_key,
            request.error,
            request.now_ms,
        )
        .await?;

        let outbox_id = if duplicate {
            None
        } else {
            append_audit_event_in_tx(&mut tx, request.event, Some(&operation.operation_id)).await?;
            Some(enqueue_outbox_in_tx(&mut tx, request.outbox).await?)
        };
        tx.commit().await?;

        Ok(PostgresExecutionUnitOfWorkReport {
            operation,
            outbox_id,
            duplicate,
        })
    }
}

impl PostgresTokenRefreshUnitOfWork {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn apply_planned_command_with_audit(
        &self,
        planned: TokenRefreshPlannedCommand,
        audit_context: TokenRefreshAuditContext,
    ) -> PgRepositoryResult<PostgresTokenRefreshUnitOfWorkReport> {
        validate_token_refresh_plan(&planned)?;
        let summary = planned
            .report
            .audit_summary(TokenRefreshReportStatus::ConflictNoop);
        self.apply_command_with_summary(planned.command, summary, audit_context)
            .await
    }

    async fn apply_command_with_summary(
        &self,
        command: TokenRefreshRepositoryCommand,
        summary: TokenRefreshAuditSummary,
        audit_context: TokenRefreshAuditContext,
    ) -> PgRepositoryResult<PostgresTokenRefreshUnitOfWorkReport> {
        let mut tx = self.pool.begin().await?;
        let apply_result = apply_refresh_command_in_tx(&mut tx, command).await?;
        let mut summary = summary;
        summary.status = if apply_result.is_some() {
            TokenRefreshReportStatus::Succeeded
        } else {
            TokenRefreshReportStatus::ConflictNoop
        };

        let event = token_refresh_audit_event(audit_context, &summary);
        append_audit_event_in_tx(&mut tx, &event, None).await?;
        tx.commit().await?;

        Ok(PostgresTokenRefreshUnitOfWorkReport {
            apply_result,
            event,
        })
    }
}

impl<A> PostgresTokenRefreshOrchestrator<A>
where
    A: AuthRefreshAdapter,
{
    pub fn new(pool: PgPool, adapter: A) -> Self {
        Self {
            adapter,
            uow: PostgresTokenRefreshUnitOfWork::new(pool.clone()),
            audit: PostgresAuditEventRepository::new(pool),
        }
    }

    pub fn adapter(&self) -> &A {
        &self.adapter
    }

    pub async fn refresh_grant_with_audit(
        &mut self,
        snapshot: TokenRefreshGrantSnapshot,
        now: SystemTime,
        audit_context: TokenRefreshAuditContext,
    ) -> PgRepositoryResult<PostgresTokenRefreshOrchestratorReport> {
        if let Some(service_report) = token_refresh_short_circuit_report(&snapshot) {
            let event = token_refresh_audit_event(audit_context, &service_report.audit_summary());
            self.audit.append(&event, None).await?;
            return Ok(PostgresTokenRefreshOrchestratorReport {
                service_report,
                event,
            });
        }

        let outcome = self.adapter.refresh(&snapshot);
        let planned = plan_token_refresh_command(&snapshot, outcome, now)
            .map_err(PostgresRepositoryError::TokenRefreshDecisionBridge)?;
        let report_template = planned.report.clone();

        let uow_report = self
            .uow
            .apply_planned_command_with_audit(planned, audit_context)
            .await?;
        let service_report = report_template.into_service_report(uow_report.apply_result.is_some());

        Ok(PostgresTokenRefreshOrchestratorReport {
            service_report,
            event: uow_report.event,
        })
    }
}

impl<A> PostgresTokenRefreshSweep<A>
where
    A: AuthRefreshAdapter,
{
    pub fn new(pool: PgPool, adapter: A) -> Self {
        Self {
            candidates: PostgresTokenGrantRepository::new(pool.clone()),
            orchestrator: PostgresTokenRefreshOrchestrator::new(pool, adapter),
        }
    }

    pub fn adapter(&self) -> &A {
        self.orchestrator.adapter()
    }

    pub async fn run_once_for_tenant(
        &mut self,
        request: PostgresTokenRefreshSweepRequest,
    ) -> PgRepositoryResult<PostgresTokenRefreshSweepReport> {
        if request.limit == 0 {
            return Ok(PostgresTokenRefreshSweepReport {
                candidate_count: 0,
                attempted_count: 0,
                reports: Vec::new(),
            });
        }

        let candidates = self
            .candidates
            .list_refresh_candidate_snapshots(&request.tenant_id, request.due_before, request.limit)
            .await?;
        let candidate_count = candidates.len();
        let mut reports = Vec::with_capacity(candidate_count);

        for (index, snapshot) in candidates.into_iter().enumerate() {
            let audit_context = TokenRefreshAuditContext {
                trace_id: request.audit_trace_id.clone(),
                sequence: request.audit_sequence_start + index as u64,
                occurred_at_ms: request.occurred_at_ms,
                actor: request.actor.clone(),
                workspace_id: request.workspace_id.clone(),
            };

            let report = self
                .orchestrator
                .refresh_grant_with_audit(snapshot, request.now, audit_context)
                .await?;
            reports.push(report);
        }

        Ok(PostgresTokenRefreshSweepReport {
            candidate_count,
            attempted_count: reports.len(),
            reports,
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
                Err(PostgresRepositoryError::InvalidOperationStatusTransition {
                    from: record.status,
                    to: target_status,
                })
            }
        }
        None => Err(PostgresRepositoryError::UnknownOperationIdempotencyKey(
            idempotency_key.to_string(),
        )),
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

fn validate_token_refresh_plan(planned: &TokenRefreshPlannedCommand) -> PgRepositoryResult<()> {
    let expected_command_kind = planned.command.kind();
    if planned.report.command_kind != expected_command_kind {
        return Err(PostgresRepositoryError::TokenRefreshPlanMismatch {
            field: "command_kind",
            expected: format!("{expected_command_kind:?}"),
            actual: format!("{:?}", planned.report.command_kind),
        });
    }

    if planned.report.tenant_id != *planned.tenant_id() {
        return Err(PostgresRepositoryError::TokenRefreshPlanMismatch {
            field: "tenant_id",
            expected: planned.tenant_id().0.clone(),
            actual: planned.report.tenant_id.0.clone(),
        });
    }

    if planned.report.grant_id != *planned.grant_id() {
        return Err(PostgresRepositoryError::TokenRefreshPlanMismatch {
            field: "grant_id",
            expected: planned.grant_id().0.clone(),
            actual: planned.report.grant_id.0.clone(),
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

async fn apply_refresh_command_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    command: TokenRefreshRepositoryCommand,
) -> PgRepositoryResult<Option<TokenRefreshApplyResult>> {
    let row = match command {
        TokenRefreshRepositoryCommand::RotateGrantCas {
            grant_id,
            tenant_id,
            expected_fingerprint,
            expires_at_ms,
            refreshed_at_ms,
            encrypted_grant_blob,
            grant_key_id,
            new_fingerprint,
        } => {
            sqlx::query(ROTATE_TOKEN_GRANT)
                .bind(&tenant_id.0)
                .bind(&grant_id.0)
                .bind(&expected_fingerprint)
                .bind(option_u64_to_i64(expires_at_ms))
                .bind(refreshed_at_ms as i64)
                .bind(&encrypted_grant_blob.0)
                .bind(&grant_key_id)
                .bind(&new_fingerprint)
                .fetch_optional(&mut **tx)
                .await?
        }
        TokenRefreshRepositoryCommand::MarkNeedsRefresh {
            grant_id,
            tenant_id,
            expected_fingerprint,
            refreshed_at_ms,
            safe_error,
        } => {
            let safe_error = sanitize_refresh_error_for_storage(&safe_error);
            sqlx::query(MARK_TOKEN_GRANT_REFRESH_FAILED)
                .bind(&tenant_id.0)
                .bind(&grant_id.0)
                .bind(&expected_fingerprint)
                .bind(refreshed_at_ms as i64)
                .bind(&safe_error)
                .fetch_optional(&mut **tx)
                .await?
        }
        TokenRefreshRepositoryCommand::MarkReauthRequired {
            grant_id,
            tenant_id,
            expected_fingerprint,
            reauth_required_at_ms,
            safe_error,
        } => {
            let safe_error = sanitize_refresh_error_for_storage(&safe_error);
            sqlx::query(MARK_TOKEN_GRANT_REAUTH_REQUIRED)
                .bind(&tenant_id.0)
                .bind(&grant_id.0)
                .bind(&expected_fingerprint)
                .bind(reauth_required_at_ms as i64)
                .bind(&safe_error)
                .fetch_optional(&mut **tx)
                .await?
        }
    };

    row.as_ref()
        .map(encrypted_token_grant_from_row)
        .transpose()
        .map(|value| value.map(token_refresh_apply_result_from_record))
}

fn token_refresh_apply_result_from_record(
    record: EncryptedTokenGrantRecord,
) -> TokenRefreshApplyResult {
    TokenRefreshApplyResult {
        grant_id: crate::domain::identity::TokenGrantId(record.id),
        tenant_id: crate::domain::identity::TenantId(record.tenant_id),
        state: record.state,
        fingerprint: record.oauth_grant_fingerprint,
    }
}

fn is_unique_violation(error: &sqlx::Error) -> bool {
    match error.as_database_error() {
        Some(db_error) => db_error
            .code()
            .map(|code| code.as_ref() == "23505")
            .unwrap_or(false),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_error_sanitizer_redacts_token_like_payloads() {
        assert_eq!(
            sanitize_refresh_error_for_storage(
                "invalid_grant: refresh_token=rt_fake Authorization: Bearer at_fake"
            ),
            REDACTED_REFRESH_ERROR
        );
        assert_eq!(
            sanitize_refresh_error_for_storage("client_secret leaked in oauth response"),
            REDACTED_REFRESH_ERROR
        );
    }

    #[test]
    fn refresh_error_sanitizer_trims_controls_and_truncates() {
        let noisy = format!(
            "  transient\nfailure\t{}  ",
            "x".repeat(MAX_REFRESH_ERROR_CHARS)
        );
        let sanitized = sanitize_refresh_error_for_storage(&noisy);

        assert!(!sanitized.contains('\n'));
        assert_eq!(sanitized.chars().count(), MAX_REFRESH_ERROR_CHARS);
        assert!(sanitized.starts_with("transient failure"));
    }
}

use crate::action::confirmed_action::ActionStatus;
use crate::domain::identity::ActorKind;
use crate::domain::token_refresh::bridge::TokenRefreshBridgeError;
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
    #[error("unknown tenant status from database: {0}")]
    UnknownTenantStatus(String),
    #[error("unknown workspace user status from database: {0}")]
    UnknownWorkspaceUserStatus(String),
    #[error("unknown identity actor kind from database: {0}")]
    UnknownIdentityActorKind(String),
    #[error("unknown scope boundary from database: {0}")]
    UnknownScopeBoundary(String),
    #[error("unknown evidence source kind from database: {0}")]
    UnknownEvidenceSourceKind(String),
    #[error("unknown evidence visibility scope from database: {0}")]
    UnknownEvidenceVisibilityScope(String),
    #[error("unknown proposed action status from database: {0}")]
    UnknownProposedActionStatus(String),
    #[error("unknown proposed action kind from database: {0}")]
    UnknownProposedActionKind(String),
    #[error("unknown risk severity from database: {0}")]
    UnknownRiskSeverity(String),
    #[error("unknown proposed action decision from database: {0}")]
    UnknownProposedActionDecision(String),
    #[error("unknown review inbox ledger stage from database: {0}")]
    UnknownReviewInboxLedgerStage(String),
    #[error("unknown review inbox ledger status from database: {0}")]
    UnknownReviewInboxLedgerStatus(String),
    #[error("unknown review inbox item status from database: {0}")]
    UnknownReviewInboxItemStatus(String),
    #[error("unknown scheduler job kind from database: {0}")]
    UnknownSchedulerJobKind(String),
    #[error("unknown scheduler job status from database: {0}")]
    UnknownSchedulerJobStatus(String),
    #[error("scheduler job safe error code contains sensitive marker")]
    UnsafeSchedulerJobErrorCode,
    #[error("audit outbox payload contains unsafe content")]
    UnsafeAuditOutboxPayload,
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
    #[error("invalid execution queue row for {field}: {reason}")]
    InvalidExecutionQueueRow {
        field: &'static str,
        reason: &'static str,
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
    #[error("review decision request mismatch for {field}: expected {expected}, got {actual}")]
    ReviewDecisionRequestMismatch {
        field: &'static str,
        expected: String,
        actual: String,
    },
    #[error("confirm/edit decision requires a confirmed action")]
    MissingConfirmedActionForDecision,
    #[error("confirm/edit decision requires a confirmation timestamp")]
    MissingConfirmedAtForDecision,
    #[error("confirm/edit decision requires an operation id")]
    MissingOperationIdForDecision,
    #[error("reject decision must not include a confirmed action")]
    UnexpectedConfirmedActionForDecision,
    #[error("reject decision must not include an operation id")]
    UnexpectedOperationIdForDecision,
}

pub type PgRepositoryResult<T> = Result<T, PostgresRepositoryError>;

pub(super) const REDACTED_TENANT_ACTUAL: &str = "<redacted>";
pub(super) const REDACTED_REFRESH_ERROR: &str = "<redacted refresh error>";
pub(super) const MAX_REFRESH_ERROR_CHARS: usize = 256;

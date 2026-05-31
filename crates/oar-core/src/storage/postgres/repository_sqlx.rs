use crate::action::audit_event::{AuditActor, AuditEvent, AuditScope, AuditTarget};
use crate::action::confirmed_action::{ActionStatus, ConfirmedAction};
use crate::action::operation_ledger::{LedgerError, OperationRecord, SubmitResult};
use crate::action::token_refresh_audit::{token_refresh_audit_event, TokenRefreshAuditContext};
use crate::domain::evidence::{EvidenceItem, EvidenceSourceKind, EvidenceVisibilityScope};
use crate::domain::identity::{
    ActorKind, LarkIdentity, ScopeBoundary, Tenant, TenantStatus, TokenGrantState, WorkspaceUser,
    WorkspaceUserStatus,
};
use crate::domain::proposed_action::{
    ProposedAction, ProposedActionDecision, ProposedActionKind, ProposedActionStatus, RiskSeverity,
};
use crate::domain::review_inbox::{ReviewInboxItem, ReviewInboxItemStatus};
use crate::domain::scheduler::{
    SchedulerJobKind, SchedulerJobLease, SchedulerJobStatus, SchedulerLeaseAcquire,
};
use crate::domain::token_refresh::bridge::plan_token_refresh_command;
use crate::domain::token_refresh::service::{
    token_refresh_short_circuit_report, AsyncAuthRefreshAdapter,
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
    GET_DEVICE_SESSION_BY_SESSION_ID, REVOKE_DEVICE_SESSION, UPSERT_DEVICE_SESSION,
};
use crate::storage::postgres::identity_sql::{
    GET_LARK_IDENTITY_BY_ACTOR_EXTERNAL, GET_LARK_IDENTITY_BY_ID, GET_TENANT_BY_ID,
    GET_WORKSPACE_USER_BY_ID, LIST_ACTIVE_TENANT_IDS, UPSERT_LARK_IDENTITY, UPSERT_TENANT,
    UPSERT_WORKSPACE_USER,
};
use crate::storage::postgres::operation_ledger_sql::{
    GET_BY_IDEMPOTENCY_KEY, LIST_CONFIRMED_ACTIONS_READY_FOR_EXECUTION, MARK_EXECUTING,
    MARK_FAILED, MARK_SUCCEEDED, SUBMIT_CONFIRMED_ACTION_AND_LEDGER,
};
use crate::storage::postgres::review_inbox_sql::{
    INSERT_EVIDENCE_ITEM, INSERT_PROPOSED_ACTION, INSERT_PROPOSED_ACTION_DECISION,
    INSERT_PROPOSED_ACTION_EVIDENCE_REF, LIST_REVIEW_INBOX_ACTIONS_FOR_SNAPSHOT,
    LIST_REVIEW_INBOX_EVIDENCE_FOR_SNAPSHOT, LIST_REVIEW_INBOX_ITEMS,
    LIST_REVIEW_INBOX_LEDGER_EVENTS_FOR_SNAPSHOT, LOAD_PROPOSED_ACTION_DECISION_FOR_RECORDER,
    LOAD_REVIEW_DECISION_ACTION, LOAD_REVIEW_DECISION_EVIDENCE, LOAD_REVIEW_DECISION_ITEM,
    UPDATE_REVIEW_INBOX_DECISION_STATE, UPDATE_REVIEW_INBOX_LEDGER_PROJECTION,
    UPSERT_REVIEW_INBOX_ITEM,
};
use crate::storage::postgres::scheduler_sql::{
    CLAIM_SCHEDULER_JOB, COMPLETE_SCHEDULER_JOB_FOR_LEASE, FAIL_SCHEDULER_JOB_FOR_LEASE,
    GET_SCHEDULER_JOB, INSERT_SCHEDULER_JOB_IF_MISSING, UPSERT_SCHEDULER_JOB,
};
use crate::storage::postgres::token_grant_sql::{
    GET_TOKEN_GRANT_BY_ID, LIST_TOKEN_REFRESH_CANDIDATE_SNAPSHOTS,
    MARK_TOKEN_GRANT_REAUTH_REQUIRED, MARK_TOKEN_GRANT_REFRESH_FAILED, REVOKE_TOKEN_GRANT,
    ROTATE_TOKEN_GRANT, UPSERT_TOKEN_GRANT,
};
use serde_json::Value;
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::fmt;
use std::time::SystemTime;

mod action_execution;
mod audit;
mod codec;
mod error;
mod identity;
mod repositories;
mod review_inbox;
mod rows;
mod scheduler;
mod token_refresh;
mod types;
mod util;

use codec::*;
pub use error::{PgRepositoryResult, PostgresRepositoryError};
use error::{MAX_REFRESH_ERROR_CHARS, REDACTED_REFRESH_ERROR, REDACTED_TENANT_ACTUAL};
pub use repositories::{
    PostgresAuditEventRepository, PostgresDeviceSessionRepository, PostgresExecutionRecorder,
    PostgresIdentityRepository, PostgresLarkIdentityRepository, PostgresOperationLedgerRepository,
    PostgresReviewDecisionRecorder, PostgresReviewInboxRepository, PostgresSchedulerJobRepository,
    PostgresTenantRepository, PostgresTokenGrantRepository, PostgresTokenRefreshOrchestrator,
    PostgresTokenRefreshRecorder, PostgresTokenRefreshSweep, PostgresWorkspaceUserRepository,
};
use rows::*;
pub use types::{
    AuditOutboxEnvelope, AuditOutboxMessage, EncryptedTokenGrantRecord,
    InsertProposedActionDecisionRequest, PostgresExecutionRecorderReport,
    PostgresReviewDecisionContextRequest, PostgresReviewDecisionRecorderReport,
    PostgresReviewDecisionRecorderRequest, PostgresTokenRefreshOrchestratorReport,
    PostgresTokenRefreshRecorderReport, PostgresTokenRefreshSweepReport,
    PostgresTokenRefreshSweepRequest, RotateEncryptedGrantRequest, StoredDeviceSession,
    StoredEvidenceItem, StoredLarkIdentity, StoredPendingConfirmedAction, StoredProposedAction,
    StoredProposedActionDecision, StoredProposedActionDecisionKind, StoredReviewDecisionContext,
    StoredReviewInboxAction, StoredReviewInboxActionDecision, StoredReviewInboxEvidence,
    StoredReviewInboxItem, StoredReviewInboxLedgerEvent, StoredReviewInboxLedgerStage,
    StoredReviewInboxLedgerStatus, StoredReviewInboxSnapshot, StoredSchedulerJob, StoredTenant,
    StoredWorkspaceUser,
};
use util::*;

#[cfg(test)]
use audit::validate_audit_outbox_payload;
#[cfg(test)]
use scheduler::ensure_scheduler_safe_error_code;

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

    #[test]
    fn encrypted_token_grant_record_debug_redacts_sensitive_material() {
        let record = EncryptedTokenGrantRecord {
            id: "grant_1".to_string(),
            tenant_id: "tenant_1".to_string(),
            identity_id: "identity_1".to_string(),
            actor_kind: ActorKind::User,
            scope_boundary: ScopeBoundary::Tenant,
            scopes: vec!["okr:read".to_string()],
            state: TokenGrantState::Valid,
            issued_at_ms: 1,
            expires_at_ms: Some(2),
            refreshed_at_ms: Some(3),
            revoked_at_ms: None,
            reauth_required_at_ms: None,
            last_refresh_error: None,
            encrypted_oauth_grant: vec![1, 2, 3, 4],
            oauth_grant_key_id: "key_sensitive".to_string(),
            oauth_grant_fingerprint: "fp_sensitive".to_string(),
            revocation_reason: None,
        };

        let debug = format!("{record:?}");
        assert!(!debug.contains("key_sensitive"));
        assert!(!debug.contains("fp_sensitive"));
        assert!(!debug.contains("[1, 2, 3, 4]"));
        assert!(debug.contains("bytes=4"));
        assert!(debug.contains("[REDACTED]"));
    }

    #[test]
    fn rotate_encrypted_grant_request_debug_redacts_sensitive_material() {
        let bytes = [9_u8, 8_u8, 7_u8];
        let request = RotateEncryptedGrantRequest {
            tenant_id: "tenant_1",
            id: "grant_1",
            expected_fingerprint: "fp_expected_sensitive",
            expires_at_ms: Some(42),
            refreshed_at_ms: 88,
            encrypted_oauth_grant: &bytes,
            oauth_grant_key_id: "key_sensitive",
            oauth_grant_fingerprint: "fp_new_sensitive",
        };

        let debug = format!("{request:?}");
        assert!(!debug.contains("fp_expected_sensitive"));
        assert!(!debug.contains("key_sensitive"));
        assert!(!debug.contains("fp_new_sensitive"));
        assert!(!debug.contains("[9, 8, 7]"));
        assert!(debug.contains("bytes=3"));
        assert!(debug.contains("[REDACTED]"));
    }

    #[test]
    fn scheduler_safe_error_code_rejects_token_like_markers() {
        assert!(ensure_scheduler_safe_error_code("transient_timeout").is_ok());
        assert!(matches!(
            ensure_scheduler_safe_error_code("refresh_token leaked"),
            Err(PostgresRepositoryError::UnsafeSchedulerJobErrorCode)
        ));
        assert!(matches!(
            ensure_scheduler_safe_error_code("fingerprint_mismatch"),
            Err(PostgresRepositoryError::UnsafeSchedulerJobErrorCode)
        ));
    }

    #[test]
    fn audit_outbox_payload_validator_accepts_minimal_safe_shapes() {
        assert!(
            validate_audit_outbox_payload(&serde_json::json!({ "trace_id": "trace_1" })).is_ok()
        );
        assert!(validate_audit_outbox_payload(&serde_json::json!({
            "event_id": "evt_1",
            "trace_id": "trace_1",
            "sequence": 1,
            "tenant_id": "tenant_1",
        }))
        .is_ok());
        assert!(validate_audit_outbox_payload(&serde_json::json!({
            "kind": "update_kr_progress",
        }))
        .is_ok());
        assert!(validate_audit_outbox_payload(&serde_json::json!({
            "trace_id": "trace_token_refresh_sweep_success",
            "kind": "token_refresh_sweep",
        }))
        .is_ok());
    }

    #[test]
    fn audit_outbox_payload_validator_rejects_non_object_payload() {
        assert!(matches!(
            validate_audit_outbox_payload(&serde_json::json!("trace_1")),
            Err(PostgresRepositoryError::UnsafeAuditOutboxPayload)
        ));
    }

    #[test]
    fn audit_outbox_payload_validator_rejects_sensitive_markers_recursively() {
        assert!(matches!(
            validate_audit_outbox_payload(&serde_json::json!({
                "trace_id": "trace_sensitive",
                "event_type": "Authorization: Bearer abc123"
            })),
            Err(PostgresRepositoryError::UnsafeAuditOutboxPayload)
        ));
        assert!(matches!(
            validate_audit_outbox_payload(&serde_json::json!({
                "trace_id": "token=abc123",
            })),
            Err(PostgresRepositoryError::UnsafeAuditOutboxPayload)
        ));
        assert!(matches!(
            validate_audit_outbox_payload(&serde_json::json!({
                "trace_id": "trace_sensitive",
                "event_type": "encrypted blob"
            })),
            Err(PostgresRepositoryError::UnsafeAuditOutboxPayload)
        ));
    }

    #[test]
    fn audit_outbox_payload_validator_rejects_unknown_or_nested_payload() {
        assert!(matches!(
            validate_audit_outbox_payload(&serde_json::json!({
                "trace_id": "trace_1",
                "meta": "weekly"
            })),
            Err(PostgresRepositoryError::UnsafeAuditOutboxPayload)
        ));
        assert!(matches!(
            validate_audit_outbox_payload(&serde_json::json!({
                "trace_id": "trace_1",
                "kind": { "nested": "maintenance" }
            })),
            Err(PostgresRepositoryError::UnsafeAuditOutboxPayload)
        ));
    }
}

use super::repository_sqlx::PostgresRepositoryError;

pub(super) fn postgres_repository_safe_error(
    prefix: &'static str,
    error: &PostgresRepositoryError,
) -> String {
    format!(
        "{}: {}",
        prefix,
        postgres_repository_safe_error_reason(error)
    )
}

pub fn postgres_repository_safe_error_reason(error: &PostgresRepositoryError) -> &'static str {
    match error {
        PostgresRepositoryError::Sqlx(_) => "postgres_query_failed",
        PostgresRepositoryError::UnknownActionStatus(_) => "unknown_action_status",
        PostgresRepositoryError::UnknownAuditActorKind(_) => "unknown_audit_actor_kind",
        PostgresRepositoryError::UnknownAuditEventType(_) => "unknown_audit_event_type",
        PostgresRepositoryError::UnknownExecutionStatus(_) => "unknown_execution_status",
        PostgresRepositoryError::UnknownDeviceEntryPoint(_) => "unknown_device_entry_point",
        PostgresRepositoryError::UnknownDeviceSessionState(_) => "unknown_device_session_state",
        PostgresRepositoryError::UnknownTokenGrantState(_) => "unknown_token_grant_state",
        PostgresRepositoryError::UnknownTenantStatus(_) => "unknown_tenant_status",
        PostgresRepositoryError::UnknownWorkspaceUserStatus(_) => "unknown_workspace_user_status",
        PostgresRepositoryError::UnknownIdentityActorKind(_) => "unknown_identity_actor_kind",
        PostgresRepositoryError::UnknownScopeBoundary(_) => "unknown_scope_boundary",
        PostgresRepositoryError::UnknownEvidenceSourceKind(_) => "unknown_evidence_source_kind",
        PostgresRepositoryError::UnknownEvidenceVisibilityScope(_) => {
            "unknown_evidence_visibility_scope"
        }
        PostgresRepositoryError::UnknownProposedActionStatus(_) => "unknown_proposed_action_status",
        PostgresRepositoryError::UnknownProposedActionKind(_) => "unknown_proposed_action_kind",
        PostgresRepositoryError::UnknownRiskSeverity(_) => "unknown_risk_severity",
        PostgresRepositoryError::UnknownProposedActionDecision(_) => {
            "unknown_proposed_action_decision"
        }
        PostgresRepositoryError::UnknownReviewInboxLedgerStage(_) => {
            "unknown_review_inbox_ledger_stage"
        }
        PostgresRepositoryError::UnknownReviewInboxLedgerStatus(_) => {
            "unknown_review_inbox_ledger_status"
        }
        PostgresRepositoryError::UnknownReviewInboxItemStatus(_) => {
            "unknown_review_inbox_item_status"
        }
        PostgresRepositoryError::UnknownSchedulerJobKind(_) => "unknown_scheduler_job_kind",
        PostgresRepositoryError::UnknownSchedulerJobStatus(_) => "unknown_scheduler_job_status",
        PostgresRepositoryError::UnsafeSchedulerJobErrorCode => "unsafe_scheduler_job_error_code",
        PostgresRepositoryError::UnsafeAuditOutboxPayload => "unsafe_audit_outbox_payload",
        PostgresRepositoryError::ActionNotConfirmed(_) => "action_not_confirmed",
        PostgresRepositoryError::TenantMismatch { .. } => "tenant_mismatch",
        PostgresRepositoryError::LarkIdentityActorExternalBindingConflict { .. } => {
            "lark_identity_actor_external_binding_conflict"
        }
        PostgresRepositoryError::NegativeInteger { .. } => "negative_integer",
        PostgresRepositoryError::Json(_) => "invalid_json_payload",
        PostgresRepositoryError::TokenRefreshDecisionBridge(_) => {
            "token_refresh_decision_bridge_failed"
        }
        PostgresRepositoryError::InvalidOperationStatusTransition { .. } => {
            "invalid_operation_status_transition"
        }
        PostgresRepositoryError::InvalidExecutionQueueRow { .. } => "invalid_execution_queue_row",
        PostgresRepositoryError::UnknownOperationIdempotencyKey(_) => {
            "unknown_operation_idempotency_key"
        }
        PostgresRepositoryError::TokenRefreshPlanMismatch { .. } => "token_refresh_plan_mismatch",
        PostgresRepositoryError::ReviewDecisionRequestMismatch { .. } => {
            "review_decision_request_mismatch"
        }
        PostgresRepositoryError::MissingConfirmedActionForDecision => {
            "missing_confirmed_action_for_decision"
        }
        PostgresRepositoryError::MissingConfirmedAtForDecision => {
            "missing_confirmed_at_for_decision"
        }
        PostgresRepositoryError::MissingOperationIdForDecision => {
            "missing_operation_id_for_decision"
        }
        PostgresRepositoryError::UnexpectedConfirmedActionForDecision => {
            "unexpected_confirmed_action_for_decision"
        }
        PostgresRepositoryError::UnexpectedOperationIdForDecision => {
            "unexpected_operation_id_for_decision"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postgres_repository_safe_error_reason_does_not_echo_raw_error_text() {
        let error = PostgresRepositoryError::UnknownTenantStatus(
            "raw tenant status with password".to_string(),
        );

        let safe = postgres_repository_safe_error_reason(&error);

        assert_eq!(safe, "unknown_tenant_status");
        assert!(!safe.contains("password"));
        assert!(!safe.contains("raw tenant status"));
    }

    #[test]
    fn postgres_repository_safe_error_reason_does_not_echo_raw_sqlx_text() {
        let error = PostgresRepositoryError::Sqlx(sqlx::Error::Protocol(
            "database detail with password".to_string(),
        ));

        let safe = postgres_repository_safe_error_reason(&error);

        assert_eq!(safe, "postgres_query_failed");
        assert!(!safe.contains("password"));
        assert!(!safe.contains("database detail"));
    }

    #[test]
    fn postgres_repository_safe_error_prefixes_internal_stage_context() {
        let error = PostgresRepositoryError::UnknownTenantStatus(
            "raw tenant status with password".to_string(),
        );

        assert_eq!(
            postgres_repository_safe_error("tenant_maintenance_stage_failed", &error),
            "tenant_maintenance_stage_failed: unknown_tenant_status"
        );
    }

    #[test]
    fn postgres_repository_safe_error_reason_handles_invalid_execution_queue_rows() {
        let error = PostgresRepositoryError::InvalidExecutionQueueRow {
            field: "edited_payload",
            reason: "raw corrupted row detail",
        };

        assert_eq!(
            postgres_repository_safe_error_reason(&error),
            "invalid_execution_queue_row"
        );
    }
}

use oar_core::storage::postgres::{
    PostgresRepositoryError, PostgresTenantMaintenanceReport, PostgresTenantMaintenanceWorker,
    PostgresTenantRepository,
};

use super::types::{
    RuntimeTenantDiscovery, RuntimeTenantDiscoveryFuture, RuntimeTick, RuntimeTickFuture,
};

pub struct PostgresRuntimeTenantDiscovery {
    repository: PostgresTenantRepository,
}

impl PostgresRuntimeTenantDiscovery {
    pub fn new(repository: PostgresTenantRepository) -> Self {
        Self { repository }
    }

    pub(super) fn map_safe_error(error: &PostgresRepositoryError) -> String {
        postgres_repository_safe_error("tenant_discovery_failed", error)
    }
}

impl RuntimeTenantDiscovery for PostgresRuntimeTenantDiscovery {
    type Error = PostgresRepositoryError;

    fn discover_tenant_ids(&mut self) -> RuntimeTenantDiscoveryFuture<'_, Self::Error> {
        Box::pin(async move { self.repository.list_active_ids().await })
    }

    fn safe_error(error: &Self::Error) -> String {
        Self::map_safe_error(error)
    }
}

impl<R, D, C> RuntimeTick for PostgresTenantMaintenanceWorker<R, D, C>
where
    R: oar_core::domain::token_refresh::service::AsyncAuthRefreshAdapter + Send,
    D: oar_core::storage::postgres::audit_outbox_worker::AuditOutboxDispatcher + Send,
    C: FnMut() -> u64 + Send + 'static,
{
    type Report = PostgresTenantMaintenanceReport;
    type Error = PostgresRepositoryError;

    fn tick(&mut self) -> RuntimeTickFuture<'_, Self::Report, Self::Error> {
        Box::pin(async move { self.run_once().await })
    }

    fn safe_error(error: &Self::Error) -> String {
        postgres_repository_safe_error("tenant_maintenance_runtime_tick_failed", error)
    }
}

pub(super) fn postgres_repository_safe_error(
    prefix: &str,
    error: &PostgresRepositoryError,
) -> String {
    format!(
        "{}: {}",
        prefix,
        postgres_repository_safe_error_reason(error)
    )
}

pub(super) fn postgres_repository_safe_error_reason(
    error: &PostgresRepositoryError,
) -> &'static str {
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

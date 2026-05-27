use super::{PgRepositoryResult, PostgresRepositoryError};
use crate::action::audit_event::{AuditActorKind, AuditEventType};
use crate::action::confirmed_action::ActionStatus;
use crate::domain::device_sync::{DeviceEntryPoint, SessionState};
use crate::domain::evidence::{EvidenceSourceKind, EvidenceVisibilityScope};
use crate::domain::identity::{
    ActorKind, OarUserStatus, ScopeBoundary, TenantStatus, TokenGrantState,
};
use crate::domain::proposed_action::{
    ProposedActionDecision, ProposedActionKind, ProposedActionStatus, RiskSeverity,
};
use crate::domain::review_inbox::ReviewInboxItemStatus;
use crate::domain::scheduler::{SchedulerJobKind, SchedulerJobStatus};
use serde_json::Value;

pub(super) fn action_status_from_db(value: &str) -> PgRepositoryResult<ActionStatus> {
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

pub(super) fn action_status_to_db(value: &ActionStatus) -> &'static str {
    match value {
        ActionStatus::Proposed => "proposed",
        ActionStatus::Confirmed => "confirmed",
        ActionStatus::Executing => "executing",
        ActionStatus::Succeeded => "succeeded",
        ActionStatus::Failed => "failed",
        ActionStatus::Cancelled => "cancelled",
    }
}

pub(super) fn actor_kind_to_db(kind: &ActorKind) -> &'static str {
    match kind {
        ActorKind::User => "user",
        ActorKind::Bot => "bot",
        ActorKind::App => "app",
        ActorKind::Service => "service",
    }
}

pub(super) fn identity_actor_kind_from_db(value: &str) -> PgRepositoryResult<ActorKind> {
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

pub(super) fn scope_boundary_to_db(boundary: &ScopeBoundary) -> &'static str {
    match boundary {
        ScopeBoundary::Tenant => "tenant",
        ScopeBoundary::User => "user",
        ScopeBoundary::Admin => "admin",
        ScopeBoundary::Bot => "bot",
        ScopeBoundary::Service => "service",
    }
}

pub(super) fn scope_boundary_from_db(value: &str) -> PgRepositoryResult<ScopeBoundary> {
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

pub(super) fn token_grant_state_to_db(state: &TokenGrantState) -> &'static str {
    match state {
        TokenGrantState::Valid => "valid",
        TokenGrantState::NeedsRefresh => "needs_refresh",
        TokenGrantState::Expired => "expired",
        TokenGrantState::Revoked => "revoked",
        TokenGrantState::ReauthRequired => "reauth_required",
    }
}

pub(super) fn token_grant_state_from_db(value: &str) -> PgRepositoryResult<TokenGrantState> {
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

pub(super) fn tenant_status_to_db(status: &TenantStatus) -> &'static str {
    match status {
        TenantStatus::Active => "active",
        TenantStatus::Suspended => "suspended",
    }
}

pub(super) fn tenant_status_from_db(value: &str) -> PgRepositoryResult<TenantStatus> {
    match value {
        "active" => Ok(TenantStatus::Active),
        "suspended" => Ok(TenantStatus::Suspended),
        other => Err(PostgresRepositoryError::UnknownTenantStatus(
            other.to_string(),
        )),
    }
}

pub(super) fn oar_user_status_to_db(status: &OarUserStatus) -> &'static str {
    match status {
        OarUserStatus::Active => "active",
        OarUserStatus::Disabled => "disabled",
    }
}

pub(super) fn oar_user_status_from_db(value: &str) -> PgRepositoryResult<OarUserStatus> {
    match value {
        "active" => Ok(OarUserStatus::Active),
        "disabled" => Ok(OarUserStatus::Disabled),
        other => Err(PostgresRepositoryError::UnknownOarUserStatus(
            other.to_string(),
        )),
    }
}

pub(super) fn audit_actor_kind_to_db(kind: &AuditActorKind) -> &'static str {
    match kind {
        AuditActorKind::User => "user",
        AuditActorKind::Bot => "bot",
        AuditActorKind::App => "app",
        AuditActorKind::System => "system",
        AuditActorKind::Service => "service",
    }
}

pub(super) fn audit_actor_kind_from_db(value: &str) -> PgRepositoryResult<AuditActorKind> {
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

pub(super) fn audit_event_type_to_db(event_type: &AuditEventType) -> &'static str {
    match event_type {
        AuditEventType::ProposedActionDecisionRecorded => "proposed_action_decision_recorded",
        AuditEventType::ConfirmedActionRecorded => "confirmed_action_recorded",
        AuditEventType::DryRunExecuted => "dry_run_executed",
        AuditEventType::ExecutionDenied => "execution_denied",
        AuditEventType::ExecutionSucceeded => "execution_succeeded",
        AuditEventType::ExecutionFailed => "execution_failed",
    }
}

pub(super) fn audit_event_type_from_db(value: &str) -> PgRepositoryResult<AuditEventType> {
    match value {
        "proposed_action_decision_recorded" => Ok(AuditEventType::ProposedActionDecisionRecorded),
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

pub(super) fn device_entry_point_to_db(value: &DeviceEntryPoint) -> &'static str {
    match value {
        DeviceEntryPoint::MacOs => "macos",
        DeviceEntryPoint::Ios => "ios",
        DeviceEntryPoint::Web => "web",
        DeviceEntryPoint::Lark => "lark",
    }
}

pub(super) fn device_entry_point_from_db(value: &str) -> PgRepositoryResult<DeviceEntryPoint> {
    match value {
        "macos" => Ok(DeviceEntryPoint::MacOs),
        "ios" => Ok(DeviceEntryPoint::Ios),
        "web" => Ok(DeviceEntryPoint::Web),
        "lark" => Ok(DeviceEntryPoint::Lark),
        other => Err(PostgresRepositoryError::UnknownDeviceEntryPoint(
            other.to_string(),
        )),
    }
}

pub(super) fn device_session_state_to_db(value: &SessionState) -> &'static str {
    match value {
        SessionState::Active => "active",
        SessionState::Revoked => "revoked",
        SessionState::Expired => "expired",
    }
}

pub(super) fn device_session_state_from_db(value: &str) -> PgRepositoryResult<SessionState> {
    match value {
        "active" => Ok(SessionState::Active),
        "revoked" => Ok(SessionState::Revoked),
        "expired" => Ok(SessionState::Expired),
        other => Err(PostgresRepositoryError::UnknownDeviceSessionState(
            other.to_string(),
        )),
    }
}

pub(super) fn evidence_source_kind_to_db(value: &EvidenceSourceKind) -> &'static str {
    match value {
        EvidenceSourceKind::OkrProgress => "okr_progress",
        EvidenceSourceKind::LarkMinutes => "lark_minutes",
        EvidenceSourceKind::LarkDoc => "lark_doc",
        EvidenceSourceKind::ManualReviewNote => "manual_review_note",
        EvidenceSourceKind::AuditEvent => "audit_event",
    }
}

pub(super) fn evidence_source_kind_from_db(value: &str) -> PgRepositoryResult<EvidenceSourceKind> {
    match value {
        "okr_progress" => Ok(EvidenceSourceKind::OkrProgress),
        "lark_minutes" => Ok(EvidenceSourceKind::LarkMinutes),
        "lark_doc" => Ok(EvidenceSourceKind::LarkDoc),
        "manual_review_note" => Ok(EvidenceSourceKind::ManualReviewNote),
        "audit_event" => Ok(EvidenceSourceKind::AuditEvent),
        other => Err(PostgresRepositoryError::UnknownEvidenceSourceKind(
            other.to_string(),
        )),
    }
}

pub(super) fn evidence_visibility_scope_to_db(value: &EvidenceVisibilityScope) -> &'static str {
    match value {
        EvidenceVisibilityScope::Tenant => "tenant",
        EvidenceVisibilityScope::Team => "team",
        EvidenceVisibilityScope::User => "user",
    }
}

pub(super) fn evidence_visibility_scope_from_db(
    value: &str,
) -> PgRepositoryResult<EvidenceVisibilityScope> {
    match value {
        "tenant" => Ok(EvidenceVisibilityScope::Tenant),
        "team" => Ok(EvidenceVisibilityScope::Team),
        "user" => Ok(EvidenceVisibilityScope::User),
        other => Err(PostgresRepositoryError::UnknownEvidenceVisibilityScope(
            other.to_string(),
        )),
    }
}

pub(super) fn proposed_action_status_to_db(value: &ProposedActionStatus) -> &'static str {
    match value {
        ProposedActionStatus::Draft => "draft",
        ProposedActionStatus::Published => "published",
        ProposedActionStatus::Superseded => "superseded",
        ProposedActionStatus::Withdrawn => "withdrawn",
    }
}

#[allow(dead_code)]
pub(super) fn proposed_action_status_from_db(
    value: &str,
) -> PgRepositoryResult<ProposedActionStatus> {
    match value {
        "draft" => Ok(ProposedActionStatus::Draft),
        "published" => Ok(ProposedActionStatus::Published),
        "superseded" => Ok(ProposedActionStatus::Superseded),
        "withdrawn" => Ok(ProposedActionStatus::Withdrawn),
        other => Err(PostgresRepositoryError::UnknownProposedActionStatus(
            other.to_string(),
        )),
    }
}

pub(super) fn proposed_action_kind_to_db(
    value: &ProposedActionKind,
) -> (&'static str, Option<&str>) {
    match value {
        ProposedActionKind::CreateKrProgress => ("create_kr_progress", None),
        ProposedActionKind::UpdateKrProgress => ("update_kr_progress", None),
        ProposedActionKind::DeleteKrProgressDryRun => ("delete_kr_progress_dry_run", None),
        ProposedActionKind::Custom(custom) => ("custom", Some(custom.as_str())),
    }
}

#[allow(dead_code)]
pub(super) fn proposed_action_kind_from_db(
    kind: &str,
    custom_kind: Option<&str>,
) -> PgRepositoryResult<ProposedActionKind> {
    match kind {
        "create_kr_progress" => Ok(ProposedActionKind::CreateKrProgress),
        "update_kr_progress" => Ok(ProposedActionKind::UpdateKrProgress),
        "delete_kr_progress_dry_run" => Ok(ProposedActionKind::DeleteKrProgressDryRun),
        "custom" => Ok(ProposedActionKind::Custom(
            custom_kind.unwrap_or_default().to_string(),
        )),
        other => Err(PostgresRepositoryError::UnknownProposedActionKind(
            other.to_string(),
        )),
    }
}

pub(super) fn risk_severity_to_db(value: &RiskSeverity) -> &'static str {
    match value {
        RiskSeverity::Low => "low",
        RiskSeverity::Medium => "medium",
        RiskSeverity::High => "high",
        RiskSeverity::Critical => "critical",
    }
}

#[allow(dead_code)]
pub(super) fn risk_severity_from_db(value: &str) -> PgRepositoryResult<RiskSeverity> {
    match value {
        "low" => Ok(RiskSeverity::Low),
        "medium" => Ok(RiskSeverity::Medium),
        "high" => Ok(RiskSeverity::High),
        "critical" => Ok(RiskSeverity::Critical),
        other => Err(PostgresRepositoryError::UnknownRiskSeverity(
            other.to_string(),
        )),
    }
}

pub(super) fn proposed_action_decision_to_db(
    value: &ProposedActionDecision,
) -> (&'static str, Option<&Value>) {
    match value {
        ProposedActionDecision::Confirm => ("confirm", None),
        ProposedActionDecision::EditThenConfirm { edited_payload } => {
            ("edit_then_confirm", Some(edited_payload))
        }
        ProposedActionDecision::Reject => ("reject", None),
    }
}

#[allow(dead_code)]
pub(super) fn proposed_action_decision_from_db(
    value: &str,
    edited_payload: Option<Value>,
) -> PgRepositoryResult<ProposedActionDecision> {
    match value {
        "confirm" => Ok(ProposedActionDecision::Confirm),
        "edit_then_confirm" => Ok(ProposedActionDecision::EditThenConfirm {
            edited_payload: edited_payload.unwrap_or(Value::Null),
        }),
        "reject" => Ok(ProposedActionDecision::Reject),
        other => Err(PostgresRepositoryError::UnknownProposedActionDecision(
            other.to_string(),
        )),
    }
}

pub(super) fn review_inbox_item_status_to_db(value: &ReviewInboxItemStatus) -> &'static str {
    match value {
        ReviewInboxItemStatus::Open => "open",
        ReviewInboxItemStatus::Confirmed => "confirmed",
        ReviewInboxItemStatus::Rejected => "rejected",
        ReviewInboxItemStatus::Executing => "executing",
        ReviewInboxItemStatus::Succeeded => "succeeded",
        ReviewInboxItemStatus::Failed => "failed",
        ReviewInboxItemStatus::Withdrawn => "withdrawn",
    }
}

pub(super) fn review_inbox_item_status_from_db(
    value: &str,
) -> PgRepositoryResult<ReviewInboxItemStatus> {
    match value {
        "open" => Ok(ReviewInboxItemStatus::Open),
        "confirmed" => Ok(ReviewInboxItemStatus::Confirmed),
        "rejected" => Ok(ReviewInboxItemStatus::Rejected),
        "executing" => Ok(ReviewInboxItemStatus::Executing),
        "succeeded" => Ok(ReviewInboxItemStatus::Succeeded),
        "failed" => Ok(ReviewInboxItemStatus::Failed),
        "withdrawn" => Ok(ReviewInboxItemStatus::Withdrawn),
        other => Err(PostgresRepositoryError::UnknownReviewInboxItemStatus(
            other.to_string(),
        )),
    }
}

pub(super) fn scheduler_job_kind_to_db(value: &SchedulerJobKind) -> &'static str {
    match value {
        SchedulerJobKind::TokenRefreshSweep => "token_refresh_sweep",
    }
}

pub(super) fn scheduler_job_kind_from_db(value: &str) -> PgRepositoryResult<SchedulerJobKind> {
    match value {
        "token_refresh_sweep" => Ok(SchedulerJobKind::TokenRefreshSweep),
        other => Err(PostgresRepositoryError::UnknownSchedulerJobKind(
            other.to_string(),
        )),
    }
}

pub(super) fn scheduler_job_status_from_db(value: &str) -> PgRepositoryResult<SchedulerJobStatus> {
    match value {
        "pending" => Ok(SchedulerJobStatus::Pending),
        "running" => Ok(SchedulerJobStatus::Running),
        other => Err(PostgresRepositoryError::UnknownSchedulerJobStatus(
            other.to_string(),
        )),
    }
}

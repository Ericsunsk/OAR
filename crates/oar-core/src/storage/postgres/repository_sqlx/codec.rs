use super::{PgRepositoryResult, PostgresRepositoryError};
use crate::action::audit_event::{AuditActorKind, AuditEventType};
use crate::action::confirmed_action::ActionStatus;
use crate::domain::device_sync::{DeviceEntryPoint, SessionState};
use crate::domain::identity::{
    ActorKind, OarUserStatus, ScopeBoundary, TenantStatus, TokenGrantState,
};

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
        AuditEventType::ConfirmedActionRecorded => "confirmed_action_recorded",
        AuditEventType::DryRunExecuted => "dry_run_executed",
        AuditEventType::ExecutionDenied => "execution_denied",
        AuditEventType::ExecutionSucceeded => "execution_succeeded",
        AuditEventType::ExecutionFailed => "execution_failed",
    }
}

pub(super) fn audit_event_type_from_db(value: &str) -> PgRepositoryResult<AuditEventType> {
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

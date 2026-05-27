use super::super::{PgRepositoryResult, PostgresRepositoryError};
use crate::domain::identity::{
    ActorKind, ScopeBoundary, TenantStatus, TokenGrantState, WorkspaceUserStatus,
};

pub(in crate::storage::postgres::repository_sqlx) fn actor_kind_to_db(
    kind: &ActorKind,
) -> &'static str {
    match kind {
        ActorKind::User => "user",
        ActorKind::Bot => "bot",
        ActorKind::App => "app",
        ActorKind::Service => "service",
    }
}

pub(in crate::storage::postgres::repository_sqlx) fn identity_actor_kind_from_db(
    value: &str,
) -> PgRepositoryResult<ActorKind> {
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

pub(in crate::storage::postgres::repository_sqlx) fn scope_boundary_to_db(
    boundary: &ScopeBoundary,
) -> &'static str {
    match boundary {
        ScopeBoundary::Tenant => "tenant",
        ScopeBoundary::User => "user",
        ScopeBoundary::Admin => "admin",
        ScopeBoundary::Bot => "bot",
        ScopeBoundary::Service => "service",
    }
}

pub(in crate::storage::postgres::repository_sqlx) fn scope_boundary_from_db(
    value: &str,
) -> PgRepositoryResult<ScopeBoundary> {
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

pub(in crate::storage::postgres::repository_sqlx) fn token_grant_state_to_db(
    state: &TokenGrantState,
) -> &'static str {
    match state {
        TokenGrantState::Valid => "valid",
        TokenGrantState::NeedsRefresh => "needs_refresh",
        TokenGrantState::Expired => "expired",
        TokenGrantState::Revoked => "revoked",
        TokenGrantState::ReauthRequired => "reauth_required",
    }
}

pub(in crate::storage::postgres::repository_sqlx) fn token_grant_state_from_db(
    value: &str,
) -> PgRepositoryResult<TokenGrantState> {
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

pub(in crate::storage::postgres::repository_sqlx) fn tenant_status_to_db(
    status: &TenantStatus,
) -> &'static str {
    match status {
        TenantStatus::Active => "active",
        TenantStatus::Suspended => "suspended",
    }
}

pub(in crate::storage::postgres::repository_sqlx) fn tenant_status_from_db(
    value: &str,
) -> PgRepositoryResult<TenantStatus> {
    match value {
        "active" => Ok(TenantStatus::Active),
        "suspended" => Ok(TenantStatus::Suspended),
        other => Err(PostgresRepositoryError::UnknownTenantStatus(
            other.to_string(),
        )),
    }
}

pub(in crate::storage::postgres::repository_sqlx) fn workspace_user_status_to_db(
    status: &WorkspaceUserStatus,
) -> &'static str {
    match status {
        WorkspaceUserStatus::Active => "active",
        WorkspaceUserStatus::Disabled => "disabled",
    }
}

pub(in crate::storage::postgres::repository_sqlx) fn workspace_user_status_from_db(
    value: &str,
) -> PgRepositoryResult<WorkspaceUserStatus> {
    match value {
        "active" => Ok(WorkspaceUserStatus::Active),
        "disabled" => Ok(WorkspaceUserStatus::Disabled),
        other => Err(PostgresRepositoryError::UnknownWorkspaceUserStatus(
            other.to_string(),
        )),
    }
}

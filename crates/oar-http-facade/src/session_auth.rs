use std::time::SystemTime;

use hyper::http::StatusCode;
use oar_core::domain::device_sync::SessionState;
use oar_core::storage::postgres::{PostgresDeviceSessionRepository, StoredDeviceSession};
use serde_json::json;

use crate::response::{
    invalid_oar_session, json_facade_response, service_unavailable, unauthorized, FacadeResponse,
};
use crate::OarHttpFacadeRuntime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuthenticatedContext {
    pub(crate) session_id: String,
    pub(crate) tenant_id: String,
    pub(crate) user_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OarSessionAuthError {
    MissingBearer,
    InvalidSession,
    StoreUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LogoutSessionState {
    Active(AuthenticatedContext),
    SignedOut,
}

pub(crate) fn protected_route_requires_session_store(
    authorization: Option<&str>,
    safe_message: &'static str,
) -> FacadeResponse {
    match bearer_session_id(authorization) {
        Ok(_) => service_unavailable("oar_session_verification_unavailable", safe_message),
        Err(error) => oar_session_auth_error_response(error),
    }
}

pub(crate) async fn authenticate_oar_session(
    runtime: &OarHttpFacadeRuntime,
    authorization: Option<&str>,
) -> Result<AuthenticatedContext, OarSessionAuthError> {
    let session_id = bearer_session_id(authorization)?;
    let repository = device_session_repository(runtime)?;
    let session = repository
        .get_by_session_id_for_authentication(session_id)
        .await
        .map_err(|_| OarSessionAuthError::StoreUnavailable)?
        .ok_or(OarSessionAuthError::InvalidSession)?;
    authenticated_context_from_session(&session)
}

pub(crate) async fn logout_oar_session(
    runtime: &OarHttpFacadeRuntime,
    authorization: Option<&str>,
) -> FacadeResponse {
    let session_id = match bearer_session_id(authorization) {
        Ok(session_id) => session_id,
        Err(error) => return oar_session_auth_error_response(error),
    };
    let repository = match device_session_repository(runtime) {
        Ok(repository) => repository,
        Err(error) => return oar_session_auth_error_response(error),
    };
    let session = match repository
        .get_by_session_id_for_authentication(session_id)
        .await
        .map_err(|_| OarSessionAuthError::StoreUnavailable)
    {
        Ok(Some(session)) => session,
        Ok(None) => return oar_session_auth_error_response(OarSessionAuthError::InvalidSession),
        Err(error) => return oar_session_auth_error_response(error),
    };
    let auth_context = match logout_session_state_from_session(&session) {
        Ok(LogoutSessionState::Active(context)) => context,
        Ok(LogoutSessionState::SignedOut) => return signed_out_response(),
        Err(error) => return oar_session_auth_error_response(error),
    };
    if repository
        .revoke(
            &auth_context.tenant_id,
            &auth_context.session_id,
            SystemTime::now(),
        )
        .await
        .is_err()
    {
        return service_unavailable(
            "oar_session_logout_unavailable",
            "OAR session logout is temporarily unavailable.",
        );
    }

    signed_out_response()
}

fn device_session_repository(
    runtime: &OarHttpFacadeRuntime,
) -> Result<PostgresDeviceSessionRepository, OarSessionAuthError> {
    runtime
        .session_persistence()
        .map(|persistence| PostgresDeviceSessionRepository::new(persistence.pool()))
        .ok_or(OarSessionAuthError::StoreUnavailable)
}

pub(crate) fn logout_session_state_from_session(
    session: &StoredDeviceSession,
) -> Result<LogoutSessionState, OarSessionAuthError> {
    if session.state == SessionState::Revoked || session.revoked_at.is_some() {
        return Ok(LogoutSessionState::SignedOut);
    }

    authenticated_context_from_session(session).map(LogoutSessionState::Active)
}

pub(crate) fn authenticated_context_from_session(
    session: &StoredDeviceSession,
) -> Result<AuthenticatedContext, OarSessionAuthError> {
    if session.state != SessionState::Active
        || session.revoked_at.is_some()
        || session.expired_at.is_some()
    {
        return Err(OarSessionAuthError::InvalidSession);
    }
    Ok(AuthenticatedContext {
        session_id: session.id.clone(),
        tenant_id: session.tenant_id.clone(),
        user_id: session.user_id.clone(),
    })
}

pub(crate) fn bearer_session_id(authorization: Option<&str>) -> Result<&str, OarSessionAuthError> {
    let session_id = authorization
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(OarSessionAuthError::MissingBearer)?;
    if !session_id.starts_with("oar_session_") {
        return Err(OarSessionAuthError::InvalidSession);
    }
    Ok(session_id)
}

pub(crate) fn oar_session_auth_error_response(error: OarSessionAuthError) -> FacadeResponse {
    match error {
        OarSessionAuthError::MissingBearer => unauthorized(),
        OarSessionAuthError::InvalidSession => invalid_oar_session(),
        OarSessionAuthError::StoreUnavailable => service_unavailable(
            "oar_session_verification_unavailable",
            "OAR session verification is temporarily unavailable.",
        ),
    }
}

fn signed_out_response() -> FacadeResponse {
    json_facade_response(
        StatusCode::OK,
        json!({
            "status": "signed_out"
        }),
    )
}

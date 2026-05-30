use std::time::UNIX_EPOCH;

use oar_core::domain::device_sync::SessionState;

use super::support::stored_device_session;
use crate::session_auth::{
    authenticated_context_from_session, bearer_session_id, logout_session_state_from_session,
    LogoutSessionState, OarSessionAuthError,
};

#[test]
fn bearer_session_id_requires_oar_session_prefix() {
    assert_eq!(
        bearer_session_id(Some("Bearer oar_session_abc")).expect("session"),
        "oar_session_abc"
    );
    assert_eq!(
        bearer_session_id(Some("Bearer other_token")).expect_err("invalid"),
        OarSessionAuthError::InvalidSession
    );
    assert_eq!(
        bearer_session_id(None).expect_err("missing"),
        OarSessionAuthError::MissingBearer
    );
}

#[test]
fn authenticated_context_requires_active_device_session() {
    let active = stored_device_session(SessionState::Active, None, None);
    let context = authenticated_context_from_session(&active).expect("active context");

    assert_eq!(context.session_id, "oar_session_test");
    assert_eq!(context.tenant_id, "tenant_1");
    assert_eq!(context.user_id, "user_1");

    let revoked = stored_device_session(SessionState::Revoked, Some(UNIX_EPOCH), None);
    assert_eq!(
        authenticated_context_from_session(&revoked).expect_err("revoked"),
        OarSessionAuthError::InvalidSession
    );

    let expired = stored_device_session(SessionState::Expired, None, Some(UNIX_EPOCH));
    assert_eq!(
        authenticated_context_from_session(&expired).expect_err("expired"),
        OarSessionAuthError::InvalidSession
    );
}

#[test]
fn logout_session_state_is_idempotent_for_revoked_device_session() {
    let active = stored_device_session(SessionState::Active, None, None);
    let active_state = logout_session_state_from_session(&active).expect("active logout state");
    assert!(matches!(active_state, LogoutSessionState::Active(_)));

    let revoked = stored_device_session(SessionState::Revoked, Some(UNIX_EPOCH), None);
    assert_eq!(
        logout_session_state_from_session(&revoked).expect("revoked logout state"),
        LogoutSessionState::SignedOut
    );

    let expired = stored_device_session(SessionState::Expired, None, Some(UNIX_EPOCH));
    assert_eq!(
        logout_session_state_from_session(&expired).expect_err("expired"),
        OarSessionAuthError::InvalidSession
    );
}

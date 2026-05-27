use std::time::{Duration, SystemTime};

use oar_core::domain::device_sync::{
    DeviceEntryPoint, DeviceSession, DeviceSyncError, SessionState,
};
use oar_core::domain::identity::{DeviceSessionId, TenantId, WorkspaceUserId};

fn sample_session(now: SystemTime) -> DeviceSession {
    DeviceSession::new(
        DeviceSessionId("session_sync_01".to_string()),
        TenantId("tenant_sync_01".to_string()),
        WorkspaceUserId("user_sync_01".to_string()),
        DeviceEntryPoint::MacOs,
        "okr_review_inbox",
        10,
        now,
    )
}

#[test]
fn advance_cursor_updates_cursor_and_last_seen() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let mut session = sample_session(now);
    let later = now + Duration::from_secs(60);

    session.advance_cursor(11, later).unwrap();

    assert_eq!(session.cursor.value, 11);
    assert_eq!(session.cursor.updated_at, later);
    assert_eq!(session.last_seen_at, later);
}

#[test]
fn stale_cursor_is_rejected() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let mut session = sample_session(now);
    let later = now + Duration::from_secs(60);

    let err = session.advance_cursor(10, later).unwrap_err();
    assert_eq!(
        err,
        DeviceSyncError::StaleCursor {
            current: 10,
            proposed: 10
        }
    );
}

#[test]
fn last_seen_going_backwards_is_rejected() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let mut session = sample_session(now);
    let later = now + Duration::from_secs(60);
    session.advance_cursor(11, later).unwrap();

    let err = session
        .advance_cursor(12, later - Duration::from_secs(1))
        .unwrap_err();
    assert_eq!(
        err,
        DeviceSyncError::LastSeenWentBackwards {
            current: later,
            proposed: later - Duration::from_secs(1),
        }
    );
}

#[test]
fn revoke_sets_revoked_state_and_timestamp() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let mut session = sample_session(now);
    let revoke_at = now + Duration::from_secs(120);

    session.revoke(revoke_at);

    assert_eq!(session.state, SessionState::Revoked);
    assert_eq!(session.revoked_at, Some(revoke_at));
}

#[test]
fn updates_are_blocked_after_revoke() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let mut session = sample_session(now);
    let revoke_at = now + Duration::from_secs(120);
    let later = now + Duration::from_secs(180);
    session.revoke(revoke_at);

    assert_eq!(
        session.advance_cursor(11, later),
        Err(DeviceSyncError::SessionRevoked)
    );
    assert_eq!(
        session.update_last_seen(later),
        Err(DeviceSyncError::SessionRevoked)
    );
}

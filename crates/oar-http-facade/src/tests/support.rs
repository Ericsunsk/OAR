use std::time::{SystemTime, UNIX_EPOCH};

use oar_core::domain::device_sync::{DeviceEntryPoint, SessionState};
use oar_core::storage::postgres::StoredDeviceSession;

pub(super) fn stored_device_session(
    state: SessionState,
    revoked_at: Option<SystemTime>,
    expired_at: Option<SystemTime>,
) -> StoredDeviceSession {
    StoredDeviceSession {
        id: "oar_session_test".to_string(),
        tenant_id: "tenant_1".to_string(),
        user_id: "user_1".to_string(),
        entry_point: DeviceEntryPoint::MacOs,
        state,
        sync_stream: "review_inbox".to_string(),
        sync_cursor_value: 0,
        sync_cursor_updated_at: UNIX_EPOCH,
        session_identity_hash: "hash".to_string(),
        last_seen_at: UNIX_EPOCH,
        revoked_at,
        expired_at,
    }
}

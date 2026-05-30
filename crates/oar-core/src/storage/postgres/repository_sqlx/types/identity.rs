use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredTenant {
    pub id: String,
    pub display_name: String,
    pub status: TenantStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredWorkspaceUser {
    pub id: String,
    pub tenant_id: String,
    pub display_name: String,
    pub status: WorkspaceUserStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredLarkIdentity {
    pub id: String,
    pub tenant_id: String,
    pub actor_kind: ActorKind,
    pub actor_external_id: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredDeviceSession {
    pub id: String,
    pub tenant_id: String,
    pub user_id: String,
    pub entry_point: crate::domain::device_sync::DeviceEntryPoint,
    pub state: crate::domain::device_sync::SessionState,
    pub sync_stream: String,
    pub sync_cursor_value: u64,
    pub sync_cursor_updated_at: SystemTime,
    pub session_identity_hash: String,
    pub last_seen_at: SystemTime,
    pub revoked_at: Option<SystemTime>,
    pub expired_at: Option<SystemTime>,
}

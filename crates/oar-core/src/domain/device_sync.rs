use std::time::SystemTime;

use crate::domain::identity::{DeviceSessionId, TenantId, WorkspaceUserId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceEntryPoint {
    MacOs,
    Ios,
    Web,
    Lark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Active,
    Revoked,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncCursor {
    pub stream: String,
    pub value: u64,
    pub updated_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceSession {
    pub id: DeviceSessionId,
    pub tenant_id: TenantId,
    pub user_id: WorkspaceUserId,
    pub entry_point: DeviceEntryPoint,
    pub state: SessionState,
    pub cursor: SyncCursor,
    pub last_seen_at: SystemTime,
    pub revoked_at: Option<SystemTime>,
    pub expired_at: Option<SystemTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceSyncError {
    SessionRevoked,
    SessionExpired,
    StaleCursor {
        current: u64,
        proposed: u64,
    },
    LastSeenWentBackwards {
        current: SystemTime,
        proposed: SystemTime,
    },
}

impl DeviceSession {
    pub fn new(
        id: DeviceSessionId,
        tenant_id: TenantId,
        user_id: WorkspaceUserId,
        entry_point: DeviceEntryPoint,
        stream: impl Into<String>,
        initial_cursor: u64,
        now: SystemTime,
    ) -> Self {
        Self {
            id,
            tenant_id,
            user_id,
            entry_point,
            state: SessionState::Active,
            cursor: SyncCursor {
                stream: stream.into(),
                value: initial_cursor,
                updated_at: now,
            },
            last_seen_at: now,
            revoked_at: None,
            expired_at: None,
        }
    }

    pub fn advance_cursor(
        &mut self,
        next_cursor: u64,
        now: SystemTime,
    ) -> Result<(), DeviceSyncError> {
        self.ensure_active()?;
        self.ensure_non_decreasing_last_seen(now)?;

        if next_cursor <= self.cursor.value {
            return Err(DeviceSyncError::StaleCursor {
                current: self.cursor.value,
                proposed: next_cursor,
            });
        }

        self.cursor.value = next_cursor;
        self.cursor.updated_at = now;
        self.last_seen_at = now;
        Ok(())
    }

    pub fn update_last_seen(&mut self, now: SystemTime) -> Result<(), DeviceSyncError> {
        self.ensure_active()?;
        self.ensure_non_decreasing_last_seen(now)?;
        self.last_seen_at = now;
        Ok(())
    }

    pub fn revoke(&mut self, now: SystemTime) {
        self.state = SessionState::Revoked;
        self.revoked_at = Some(now);
    }

    pub fn expire(&mut self, now: SystemTime) {
        self.state = SessionState::Expired;
        self.expired_at = Some(now);
    }

    fn ensure_active(&self) -> Result<(), DeviceSyncError> {
        match self.state {
            SessionState::Active => Ok(()),
            SessionState::Revoked => Err(DeviceSyncError::SessionRevoked),
            SessionState::Expired => Err(DeviceSyncError::SessionExpired),
        }
    }

    fn ensure_non_decreasing_last_seen(&self, now: SystemTime) -> Result<(), DeviceSyncError> {
        if now < self.last_seen_at {
            return Err(DeviceSyncError::LastSeenWentBackwards {
                current: self.last_seen_at,
                proposed: now,
            });
        }
        Ok(())
    }
}

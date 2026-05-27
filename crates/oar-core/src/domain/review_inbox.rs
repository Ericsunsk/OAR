use std::cmp::Ordering;
use std::time::SystemTime;

use crate::domain::identity::{OarUserId, TenantId};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReviewInboxItemId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewInboxItemStatus {
    Open,
    Confirmed,
    Rejected,
    Executing,
    Succeeded,
    Failed,
    Withdrawn,
}

impl ReviewInboxItemStatus {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            ReviewInboxItemStatus::Rejected
                | ReviewInboxItemStatus::Succeeded
                | ReviewInboxItemStatus::Failed
                | ReviewInboxItemStatus::Withdrawn
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewInboxItem {
    pub id: ReviewInboxItemId,
    pub tenant_id: TenantId,
    pub user_id: OarUserId,
    pub proposed_action_id: String,
    pub proposed_action_version: u64,
    pub risk_score: u32,
    pub priority: u32,
    pub status: ReviewInboxItemStatus,
    pub sort_key: i64,
    pub sync_cursor: u64,
    pub updated_at: SystemTime,
    pub ledger_status: Option<String>,
    pub operation_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewInboxError {
    StaleSyncCursor {
        current: u64,
        proposed: u64,
    },
    InvalidStatusTransition {
        from: ReviewInboxItemStatus,
        to: ReviewInboxItemStatus,
    },
    InvalidLedgerProjection {
        from: ReviewInboxItemStatus,
        to: ReviewInboxItemStatus,
    },
}

impl ReviewInboxItem {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: ReviewInboxItemId,
        tenant_id: TenantId,
        user_id: OarUserId,
        proposed_action_id: impl Into<String>,
        proposed_action_version: u64,
        risk_score: u32,
        priority: u32,
        sort_key: i64,
        sync_cursor: u64,
        updated_at: SystemTime,
    ) -> Self {
        Self {
            id,
            tenant_id,
            user_id,
            proposed_action_id: proposed_action_id.into(),
            proposed_action_version,
            risk_score,
            priority,
            status: ReviewInboxItemStatus::Open,
            sort_key,
            sync_cursor,
            updated_at,
            ledger_status: None,
            operation_id: None,
        }
    }

    pub fn confirm(
        &mut self,
        next_sync_cursor: u64,
        now: SystemTime,
    ) -> Result<(), ReviewInboxError> {
        self.transition_status(ReviewInboxItemStatus::Confirmed, next_sync_cursor, now)
    }

    pub fn reject(
        &mut self,
        next_sync_cursor: u64,
        now: SystemTime,
    ) -> Result<(), ReviewInboxError> {
        self.transition_status(ReviewInboxItemStatus::Rejected, next_sync_cursor, now)
    }

    pub fn withdraw(
        &mut self,
        next_sync_cursor: u64,
        now: SystemTime,
    ) -> Result<(), ReviewInboxError> {
        self.transition_status(ReviewInboxItemStatus::Withdrawn, next_sync_cursor, now)
    }

    pub fn advance_sync_cursor(
        &mut self,
        next_sync_cursor: u64,
        now: SystemTime,
    ) -> Result<(), ReviewInboxError> {
        self.ensure_sync_cursor_advances(next_sync_cursor)?;
        self.sync_cursor = next_sync_cursor;
        self.updated_at = now;
        Ok(())
    }

    pub fn apply_ledger_projection(
        &mut self,
        next_status: ReviewInboxItemStatus,
        ledger_status: impl Into<String>,
        operation_id: Option<String>,
        next_sync_cursor: u64,
        now: SystemTime,
    ) -> Result<(), ReviewInboxError> {
        let allowed = matches!(
            (self.status, next_status),
            (
                ReviewInboxItemStatus::Confirmed,
                ReviewInboxItemStatus::Executing
            ) | (
                ReviewInboxItemStatus::Confirmed,
                ReviewInboxItemStatus::Succeeded
            ) | (
                ReviewInboxItemStatus::Confirmed,
                ReviewInboxItemStatus::Failed
            ) | (
                ReviewInboxItemStatus::Executing,
                ReviewInboxItemStatus::Succeeded
            ) | (
                ReviewInboxItemStatus::Executing,
                ReviewInboxItemStatus::Failed
            )
        );

        if !allowed {
            return Err(ReviewInboxError::InvalidLedgerProjection {
                from: self.status,
                to: next_status,
            });
        }

        self.transition_status(next_status, next_sync_cursor, now)?;
        self.ledger_status = Some(ledger_status.into());
        self.operation_id = operation_id;
        Ok(())
    }

    pub fn cmp_for_inbox(&self, other: &Self) -> Ordering {
        other
            .sort_key
            .cmp(&self.sort_key)
            .then_with(|| other.updated_at.cmp(&self.updated_at))
            .then_with(|| self.id.0.cmp(&other.id.0))
    }

    fn transition_status(
        &mut self,
        next_status: ReviewInboxItemStatus,
        next_sync_cursor: u64,
        now: SystemTime,
    ) -> Result<(), ReviewInboxError> {
        if self.status == next_status {
            self.ensure_sync_cursor_advances(next_sync_cursor)?;
            self.sync_cursor = next_sync_cursor;
            self.updated_at = now;
            return Ok(());
        }

        let allowed = matches!(
            (self.status, next_status),
            (
                ReviewInboxItemStatus::Open,
                ReviewInboxItemStatus::Confirmed
            ) | (ReviewInboxItemStatus::Open, ReviewInboxItemStatus::Rejected)
                | (
                    ReviewInboxItemStatus::Open,
                    ReviewInboxItemStatus::Withdrawn
                )
                | (
                    ReviewInboxItemStatus::Confirmed,
                    ReviewInboxItemStatus::Executing
                )
                | (
                    ReviewInboxItemStatus::Confirmed,
                    ReviewInboxItemStatus::Succeeded
                )
                | (
                    ReviewInboxItemStatus::Confirmed,
                    ReviewInboxItemStatus::Failed
                )
                | (
                    ReviewInboxItemStatus::Confirmed,
                    ReviewInboxItemStatus::Withdrawn
                )
                | (
                    ReviewInboxItemStatus::Executing,
                    ReviewInboxItemStatus::Succeeded
                )
                | (
                    ReviewInboxItemStatus::Executing,
                    ReviewInboxItemStatus::Failed
                )
        );

        if !allowed {
            return Err(ReviewInboxError::InvalidStatusTransition {
                from: self.status,
                to: next_status,
            });
        }

        self.ensure_sync_cursor_advances(next_sync_cursor)?;
        self.status = next_status;
        self.sync_cursor = next_sync_cursor;
        self.updated_at = now;
        Ok(())
    }

    fn ensure_sync_cursor_advances(&self, proposed: u64) -> Result<(), ReviewInboxError> {
        if proposed <= self.sync_cursor {
            return Err(ReviewInboxError::StaleSyncCursor {
                current: self.sync_cursor,
                proposed,
            });
        }
        Ok(())
    }
}

use super::super::{
    PgRepositoryResult, PostgresRepositoryError, StoredReviewInboxLedgerStage,
    StoredReviewInboxLedgerStatus,
};
use crate::domain::review_inbox::ReviewInboxItemStatus;

pub(in crate::storage::postgres::repository_sqlx) fn review_inbox_item_status_to_db(
    value: &ReviewInboxItemStatus,
) -> &'static str {
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

pub(in crate::storage::postgres::repository_sqlx) fn review_inbox_item_status_from_db(
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

pub(in crate::storage::postgres::repository_sqlx) fn review_inbox_ledger_stage_from_db(
    value: &str,
) -> PgRepositoryResult<StoredReviewInboxLedgerStage> {
    match value {
        "confirmed_action" => Ok(StoredReviewInboxLedgerStage::ConfirmedAction),
        "operation_ledger" => Ok(StoredReviewInboxLedgerStage::OperationLedger),
        "platform_adapter" => Ok(StoredReviewInboxLedgerStage::PlatformAdapter),
        "audit_event" => Ok(StoredReviewInboxLedgerStage::AuditEvent),
        other => Err(PostgresRepositoryError::UnknownReviewInboxLedgerStage(
            other.to_string(),
        )),
    }
}

pub(in crate::storage::postgres::repository_sqlx) fn review_inbox_ledger_status_from_db(
    value: &str,
) -> PgRepositoryResult<StoredReviewInboxLedgerStatus> {
    match value {
        "pending" => Ok(StoredReviewInboxLedgerStatus::Pending),
        "ok" => Ok(StoredReviewInboxLedgerStatus::Ok),
        "error" => Ok(StoredReviewInboxLedgerStatus::Error),
        other => Err(PostgresRepositoryError::UnknownReviewInboxLedgerStatus(
            other.to_string(),
        )),
    }
}

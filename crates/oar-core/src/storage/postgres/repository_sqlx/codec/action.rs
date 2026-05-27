use super::super::{PgRepositoryResult, PostgresRepositoryError};
use crate::action::confirmed_action::ActionStatus;

pub(in crate::storage::postgres::repository_sqlx) fn action_status_from_db(
    value: &str,
) -> PgRepositoryResult<ActionStatus> {
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

pub(in crate::storage::postgres::repository_sqlx) fn action_status_to_db(
    value: &ActionStatus,
) -> &'static str {
    match value {
        ActionStatus::Proposed => "proposed",
        ActionStatus::Confirmed => "confirmed",
        ActionStatus::Executing => "executing",
        ActionStatus::Succeeded => "succeeded",
        ActionStatus::Failed => "failed",
        ActionStatus::Cancelled => "cancelled",
    }
}

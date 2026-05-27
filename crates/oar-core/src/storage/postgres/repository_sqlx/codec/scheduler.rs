use super::super::{PgRepositoryResult, PostgresRepositoryError};
use crate::domain::scheduler::{SchedulerJobKind, SchedulerJobStatus};

pub(in crate::storage::postgres::repository_sqlx) fn scheduler_job_kind_to_db(
    value: &SchedulerJobKind,
) -> &'static str {
    match value {
        SchedulerJobKind::TokenRefreshSweep => "token_refresh_sweep",
    }
}

pub(in crate::storage::postgres::repository_sqlx) fn scheduler_job_kind_from_db(
    value: &str,
) -> PgRepositoryResult<SchedulerJobKind> {
    match value {
        "token_refresh_sweep" => Ok(SchedulerJobKind::TokenRefreshSweep),
        other => Err(PostgresRepositoryError::UnknownSchedulerJobKind(
            other.to_string(),
        )),
    }
}

pub(in crate::storage::postgres::repository_sqlx) fn scheduler_job_status_from_db(
    value: &str,
) -> PgRepositoryResult<SchedulerJobStatus> {
    match value {
        "pending" => Ok(SchedulerJobStatus::Pending),
        "running" => Ok(SchedulerJobStatus::Running),
        other => Err(PostgresRepositoryError::UnknownSchedulerJobStatus(
            other.to_string(),
        )),
    }
}

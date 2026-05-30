use super::*;

pub(in crate::storage::postgres::repository_sqlx) fn stored_scheduler_job_from_row(
    row: &PgRow,
) -> PgRepositoryResult<StoredSchedulerJob> {
    let job_kind: String = row.try_get("job_kind")?;
    let status: String = row.try_get("status")?;
    Ok(StoredSchedulerJob {
        id: row.try_get("id")?,
        tenant_id: row.try_get("tenant_id")?,
        job_kind: scheduler_job_kind_from_db(&job_kind)?,
        status: scheduler_job_status_from_db(&status)?,
        next_run_at_ms: non_negative_i64_to_u64(row.try_get("next_run_at_ms")?, "next_run_at_ms")?,
        lease_id: row.try_get("lease_id")?,
        lease_until_ms: optional_non_negative_i64_to_u64(
            row.try_get("lease_until_ms")?,
            "lease_until_ms",
        )?,
        attempt_count: non_negative_i64_to_u64(
            row.try_get::<i32, _>("attempt_count")? as i64,
            "attempt_count",
        )? as u32,
        last_started_at_ms: optional_non_negative_i64_to_u64(
            row.try_get("last_started_at_ms")?,
            "last_started_at_ms",
        )?,
        last_finished_at_ms: optional_non_negative_i64_to_u64(
            row.try_get("last_finished_at_ms")?,
            "last_finished_at_ms",
        )?,
        last_safe_error_code: row.try_get("last_safe_error_code")?,
    })
}

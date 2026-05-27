use super::*;

impl PostgresSchedulerJobRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn upsert_job(
        &self,
        id: &str,
        tenant_id: &str,
        job_kind: SchedulerJobKind,
        next_run_at_ms: u64,
    ) -> PgRepositoryResult<StoredSchedulerJob> {
        let row = sqlx::query(UPSERT_SCHEDULER_JOB)
            .bind(id)
            .bind(tenant_id)
            .bind(scheduler_job_kind_to_db(&job_kind))
            .bind(next_run_at_ms as i64)
            .fetch_one(&self.pool)
            .await?;
        stored_scheduler_job_from_row(&row)
    }

    pub async fn get_job(
        &self,
        tenant_id: &str,
        job_kind: SchedulerJobKind,
    ) -> PgRepositoryResult<Option<StoredSchedulerJob>> {
        let row = sqlx::query(GET_SCHEDULER_JOB)
            .bind(tenant_id)
            .bind(scheduler_job_kind_to_db(&job_kind))
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(stored_scheduler_job_from_row).transpose()
    }

    pub async fn try_acquire(
        &self,
        tenant_id: &str,
        job_kind: SchedulerJobKind,
        now_ms: u64,
        lease_id: &str,
        lease_until_ms: u64,
    ) -> PgRepositoryResult<SchedulerLeaseAcquire> {
        let row = sqlx::query(CLAIM_SCHEDULER_JOB)
            .bind(tenant_id)
            .bind(scheduler_job_kind_to_db(&job_kind))
            .bind(now_ms as i64)
            .bind(lease_id)
            .bind(lease_until_ms as i64)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row {
            let stored = stored_scheduler_job_from_row(&row)?;
            return Ok(SchedulerLeaseAcquire::Acquired(
                scheduler_job_lease_from_stored(stored)?,
            ));
        }

        let Some(stored) = self.get_job(tenant_id, job_kind).await? else {
            return Ok(SchedulerLeaseAcquire::NotDue {
                next_due_ms: now_ms,
            });
        };

        match stored.status {
            SchedulerJobStatus::Running => {
                let retry_after_ms = stored
                    .lease_until_ms
                    .unwrap_or(now_ms)
                    .saturating_sub(now_ms);
                Ok(SchedulerLeaseAcquire::Busy { retry_after_ms })
            }
            SchedulerJobStatus::Pending => Ok(SchedulerLeaseAcquire::NotDue {
                next_due_ms: stored.next_run_at_ms,
            }),
        }
    }

    pub async fn complete_for_lease(
        &self,
        lease: &SchedulerJobLease,
        finished_at_ms: u64,
        next_run_at_ms: u64,
    ) -> PgRepositoryResult<bool> {
        let row = sqlx::query(COMPLETE_SCHEDULER_JOB_FOR_LEASE)
            .bind(&lease.tenant_id)
            .bind(&lease.id)
            .bind(&lease.lease_id)
            .bind(lease.attempt_count as i32)
            .bind(lease.lease_until_ms as i64)
            .bind(finished_at_ms as i64)
            .bind(next_run_at_ms as i64)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.is_some())
    }

    pub async fn fail_for_lease(
        &self,
        lease: &SchedulerJobLease,
        finished_at_ms: u64,
        safe_error_code: &str,
        next_run_at_ms: u64,
    ) -> PgRepositoryResult<bool> {
        super::scheduler::ensure_scheduler_safe_error_code(safe_error_code)?;
        let row = sqlx::query(FAIL_SCHEDULER_JOB_FOR_LEASE)
            .bind(&lease.tenant_id)
            .bind(&lease.id)
            .bind(&lease.lease_id)
            .bind(lease.attempt_count as i32)
            .bind(lease.lease_until_ms as i64)
            .bind(finished_at_ms as i64)
            .bind(safe_error_code)
            .bind(next_run_at_ms as i64)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.is_some())
    }
}

fn scheduler_job_lease_from_stored(
    stored: StoredSchedulerJob,
) -> PgRepositoryResult<SchedulerJobLease> {
    let lease_id = stored.lease_id.ok_or_else(|| {
        PostgresRepositoryError::UnknownSchedulerJobStatus("running row missing lease_id".into())
    })?;
    let lease_until_ms = stored.lease_until_ms.ok_or_else(|| {
        PostgresRepositoryError::UnknownSchedulerJobStatus("running row missing lease_until".into())
    })?;

    Ok(SchedulerJobLease {
        id: stored.id,
        tenant_id: stored.tenant_id,
        job_kind: stored.job_kind,
        status: stored.status,
        attempt_count: stored.attempt_count,
        lease_id,
        lease_until_ms,
    })
}

pub(super) fn ensure_scheduler_safe_error_code(value: &str) -> PgRepositoryResult<()> {
    let valid_shape = !value.is_empty()
        && value.len() <= 64
        && value.chars().all(|ch| {
            ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '_' | ':' | '.' | '-')
        });
    if !valid_shape {
        return Err(PostgresRepositoryError::UnsafeSchedulerJobErrorCode);
    }

    let lowered = value.to_ascii_lowercase();
    let looks_sensitive = [
        "access token",
        "access_token",
        "refresh token",
        "refresh_token",
        "authorization",
        "bearer",
        "client_secret",
        "authorization_code",
        "oauth_grant",
        "fingerprint",
        "encrypted",
    ]
    .iter()
    .any(|needle| lowered.contains(needle));

    if looks_sensitive {
        return Err(PostgresRepositoryError::UnsafeSchedulerJobErrorCode);
    }
    Ok(())
}

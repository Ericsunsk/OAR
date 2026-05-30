use super::all_sql_lowercase;

#[test]
fn scheduler_jobs_are_tenant_scoped_leased_safe_metadata() {
    let sql = all_sql_lowercase();

    assert!(
        sql.contains("create table scheduler_jobs"),
        "expected scheduler_jobs table"
    );
    assert!(sql.contains("tenant_id text not null references tenants(id)"));
    assert!(
        sql.contains("job_kind text not null check (job_kind in ('token_refresh_sweep'))"),
        "expected narrow scheduler job kind enum for Phase 0.6"
    );
    assert!(
        sql.contains("status text not null check (status in ('pending', 'running'))"),
        "expected scheduler job state guard"
    );
    assert!(sql.contains("attempt_count integer not null default 0 check (attempt_count >= 0)"));
    assert!(sql.contains("next_run_at timestamptz not null"));
    assert!(sql.contains("lease_id text"));
    assert!(sql.contains("lease_until timestamptz"));
    assert!(sql.contains("last_safe_error_code text check"));
    assert!(
        sql.contains("last_safe_error_code ~ '^[a-z0-9_:.-]{1,64}$'"),
        "expected scheduler safe error codes to be bounded machine codes"
    );
    assert!(
        sql.contains("primary key (tenant_id, job_kind)"),
        "expected one durable job row per tenant/job kind as the primary key"
    );
    assert!(
        sql.contains("unique (tenant_id, id)"),
        "expected scheduler job ids to be tenant-scoped, not globally unique"
    );
    assert!(
        sql.contains("status = 'running' and lease_id is not null and lease_until is not null")
            && sql.contains("status <> 'running' and lease_id is null and lease_until is null"),
        "expected running jobs to hold lease metadata and non-running jobs to clear it"
    );
    assert!(
        sql.contains("idx_scheduler_jobs_due"),
        "expected scheduler claim index"
    );
    assert!(
        !sql.contains("scheduler_jobs") || !sql.contains("raw_stdout"),
        "scheduler metadata must not store raw adapter stdout"
    );
    assert!(
        !sql.contains("raw_stderr"),
        "scheduler metadata must not store raw adapter stderr"
    );
}

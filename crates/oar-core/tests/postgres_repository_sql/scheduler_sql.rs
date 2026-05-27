use oar_core::storage::postgres::scheduler_sql::{
    CLAIM_SCHEDULER_JOB, COMPLETE_SCHEDULER_JOB_FOR_LEASE, FAIL_SCHEDULER_JOB_FOR_LEASE,
    GET_SCHEDULER_JOB, UPSERT_SCHEDULER_JOB,
};

use crate::compact;

#[test]
fn scheduler_job_upsert_is_tenant_kind_scoped_and_avoids_running_overwrite() {
    let sql = compact(UPSERT_SCHEDULER_JOB);

    assert!(sql.starts_with("with upserted as"));
    assert!(sql.contains("insert into scheduler_jobs"));
    assert!(sql.contains("tenant_id"));
    assert!(sql.contains("job_kind"));
    assert!(sql.contains("on conflict (tenant_id, job_kind) do update"));
    assert!(sql.contains("set id = excluded.id"));
    assert!(sql.contains("where scheduler_jobs.status <> 'running'"));
    assert!(sql.contains("not exists (select 1 from upserted)"));
    assert!(sql.contains("returning"));
}

#[test]
fn scheduler_job_get_is_tenant_kind_scoped() {
    let sql = compact(GET_SCHEDULER_JOB);

    assert!(sql.contains("from scheduler_jobs"));
    assert!(sql.contains("where tenant_id = $1"));
    assert!(sql.contains("and job_kind = $2"));
    assert!(sql.contains("limit 1"));
}

#[test]
fn scheduler_job_claim_uses_due_or_stale_rows_with_skip_locked_lease() {
    let sql = compact(CLAIM_SCHEDULER_JOB);

    assert!(sql.contains("from scheduler_jobs"));
    assert!(sql.contains("where tenant_id = $1"));
    assert!(sql.contains("and job_kind = $2"));
    assert!(sql.contains("select tenant_id, job_kind"));
    assert!(sql.contains("status in ('pending', 'running')"));
    assert!(sql.contains("status = 'pending' and next_run_at <="));
    assert!(sql.contains("status = 'running' and lease_until <="));
    assert!(sql.contains("for update skip locked"));
    assert!(sql.contains("update scheduler_jobs as sj"));
    assert!(sql.contains("sj.tenant_id = candidate.tenant_id"));
    assert!(sql.contains("sj.job_kind = candidate.job_kind"));
    assert!(sql.contains("set status = 'running'"));
    assert!(sql.contains("lease_id = $4"));
    assert!(sql.contains("lease_until = to_timestamp($5::double precision / 1000.0)"));
    assert!(sql.contains("attempt_count = attempt_count + 1"));
    assert!(sql.contains("last_safe_error_code = null"));
}

#[test]
fn scheduler_job_finalize_paths_are_attempt_and_lease_guarded() {
    let complete = compact(COMPLETE_SCHEDULER_JOB_FOR_LEASE);
    let retry = compact(FAIL_SCHEDULER_JOB_FOR_LEASE);

    for sql in [&complete, &retry] {
        assert!(sql.contains("where tenant_id = $1"));
        assert!(sql.contains("and id = $2"));
        assert!(sql.contains("and lease_id = $3"));
        assert!(sql.contains("and attempt_count = $4"));
        assert!(sql.contains("lease_until = to_timestamp"));
        assert!(sql.contains("and status = 'running'"));
        assert!(sql.contains("returning id"));
    }

    assert!(complete.contains("set status = 'pending'"));
    assert!(complete.contains("last_safe_error_code = null"));
    assert!(complete.contains("next_run_at = to_timestamp($7::double precision / 1000.0)"));
    assert!(retry.contains("set status = 'pending'"));
    assert!(retry.contains("last_safe_error_code = $7"));
    assert!(retry.contains("next_run_at = to_timestamp($8::double precision / 1000.0)"));
}

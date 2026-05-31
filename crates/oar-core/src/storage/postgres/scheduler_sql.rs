pub const UPSERT_SCHEDULER_JOB: &str = r#"
WITH upserted AS (
INSERT INTO scheduler_jobs (
    id,
    tenant_id,
    job_kind,
    status,
    next_run_at,
    attempt_count
)
VALUES (
    $1,
    $2,
    $3,
    'pending',
    to_timestamp($4::double precision / 1000.0),
    0
)
ON CONFLICT (tenant_id, job_kind) DO UPDATE
SET id = EXCLUDED.id,
    next_run_at = EXCLUDED.next_run_at,
    updated_at = now()
WHERE scheduler_jobs.status <> 'running'
RETURNING
id,
tenant_id,
job_kind,
status,
floor(extract(epoch from next_run_at) * 1000)::bigint AS next_run_at_ms,
lease_id,
floor(extract(epoch from lease_until) * 1000)::bigint AS lease_until_ms,
attempt_count,
floor(extract(epoch from last_started_at) * 1000)::bigint AS last_started_at_ms,
floor(extract(epoch from last_finished_at) * 1000)::bigint AS last_finished_at_ms,
last_safe_error_code
)
SELECT *
FROM upserted
UNION ALL
SELECT
id,
tenant_id,
job_kind,
status,
floor(extract(epoch from next_run_at) * 1000)::bigint AS next_run_at_ms,
lease_id,
floor(extract(epoch from lease_until) * 1000)::bigint AS lease_until_ms,
attempt_count,
floor(extract(epoch from last_started_at) * 1000)::bigint AS last_started_at_ms,
floor(extract(epoch from last_finished_at) * 1000)::bigint AS last_finished_at_ms,
last_safe_error_code
FROM scheduler_jobs
WHERE tenant_id = $2
  AND job_kind = $3
  AND NOT EXISTS (SELECT 1 FROM upserted)
LIMIT 1
"#;

pub const INSERT_SCHEDULER_JOB_IF_MISSING: &str = r#"
INSERT INTO scheduler_jobs (
    id,
    tenant_id,
    job_kind,
    status,
    next_run_at,
    attempt_count
)
VALUES (
    $1,
    $2,
    $3,
    'pending',
    to_timestamp($4::double precision / 1000.0),
    0
)
ON CONFLICT (tenant_id, job_kind) DO NOTHING
RETURNING
id,
tenant_id,
job_kind,
status,
floor(extract(epoch from next_run_at) * 1000)::bigint AS next_run_at_ms,
lease_id,
floor(extract(epoch from lease_until) * 1000)::bigint AS lease_until_ms,
attempt_count,
floor(extract(epoch from last_started_at) * 1000)::bigint AS last_started_at_ms,
floor(extract(epoch from last_finished_at) * 1000)::bigint AS last_finished_at_ms,
last_safe_error_code
"#;

pub const GET_SCHEDULER_JOB: &str = r#"
SELECT
id,
tenant_id,
job_kind,
status,
floor(extract(epoch from next_run_at) * 1000)::bigint AS next_run_at_ms,
lease_id,
floor(extract(epoch from lease_until) * 1000)::bigint AS lease_until_ms,
attempt_count,
floor(extract(epoch from last_started_at) * 1000)::bigint AS last_started_at_ms,
floor(extract(epoch from last_finished_at) * 1000)::bigint AS last_finished_at_ms,
last_safe_error_code
FROM scheduler_jobs
WHERE tenant_id = $1
  AND job_kind = $2
LIMIT 1
"#;

pub const CLAIM_SCHEDULER_JOB: &str = r#"
WITH candidate AS (
    SELECT tenant_id, job_kind
    FROM scheduler_jobs
    WHERE tenant_id = $1
      AND job_kind = $2
      AND status IN ('pending', 'running')
      AND (
        (status = 'pending' AND next_run_at <= to_timestamp($3::double precision / 1000.0))
        OR (status = 'running' AND lease_until <= to_timestamp($3::double precision / 1000.0))
      )
    ORDER BY next_run_at ASC, id ASC
    LIMIT 1
    FOR UPDATE SKIP LOCKED
),
claimed AS (
    UPDATE scheduler_jobs AS sj
    SET status = 'running',
        lease_id = $4,
        lease_until = to_timestamp($5::double precision / 1000.0),
        attempt_count = attempt_count + 1,
        last_started_at = to_timestamp($3::double precision / 1000.0),
        last_safe_error_code = NULL,
        updated_at = to_timestamp($3::double precision / 1000.0)
    FROM candidate
    WHERE sj.tenant_id = candidate.tenant_id
      AND sj.job_kind = candidate.job_kind
    RETURNING
        sj.id,
        sj.tenant_id,
        sj.job_kind,
        sj.status,
        floor(extract(epoch from sj.next_run_at) * 1000)::bigint AS next_run_at_ms,
        sj.lease_id,
        floor(extract(epoch from sj.lease_until) * 1000)::bigint AS lease_until_ms,
        sj.attempt_count,
        floor(extract(epoch from sj.last_started_at) * 1000)::bigint AS last_started_at_ms,
        floor(extract(epoch from sj.last_finished_at) * 1000)::bigint AS last_finished_at_ms,
        sj.last_safe_error_code
)
SELECT *
FROM claimed
LIMIT 1
"#;

pub const COMPLETE_SCHEDULER_JOB_FOR_LEASE: &str = r#"
UPDATE scheduler_jobs
SET status = 'pending',
    lease_id = NULL,
    lease_until = NULL,
    next_run_at = to_timestamp($7::double precision / 1000.0),
    last_finished_at = to_timestamp($6::double precision / 1000.0),
    last_safe_error_code = NULL,
    updated_at = to_timestamp($6::double precision / 1000.0)
WHERE tenant_id = $1
  AND id = $2
  AND lease_id = $3
  AND attempt_count = $4
  AND lease_until = to_timestamp($5::double precision / 1000.0)
  AND status = 'running'
RETURNING id
"#;

pub const FAIL_SCHEDULER_JOB_FOR_LEASE: &str = r#"
UPDATE scheduler_jobs
SET status = 'pending',
    lease_id = NULL,
    lease_until = NULL,
    next_run_at = to_timestamp($8::double precision / 1000.0),
    last_finished_at = to_timestamp($6::double precision / 1000.0),
    last_safe_error_code = $7,
    updated_at = to_timestamp($6::double precision / 1000.0)
WHERE tenant_id = $1
  AND id = $2
  AND lease_id = $3
  AND attempt_count = $4
  AND lease_until = to_timestamp($5::double precision / 1000.0)
  AND status = 'running'
RETURNING id
"#;

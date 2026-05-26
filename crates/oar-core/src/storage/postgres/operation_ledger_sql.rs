pub const SUBMIT_CONFIRMED_ACTION_AND_LEDGER: &str = r#"
WITH inserted_action AS (
    INSERT INTO confirmed_actions (
        action_id,
        tenant_id,
        actor_user_id,
        idempotency_key,
        status,
        confirmed_at
    )
    VALUES ($1, $2, $3, $4, 'confirmed', $5)
    ON CONFLICT (tenant_id, idempotency_key) DO NOTHING
    RETURNING action_id, tenant_id, idempotency_key
),
canonical_action AS (
    SELECT action_id, tenant_id, idempotency_key FROM inserted_action
    UNION ALL
    SELECT action_id, tenant_id, idempotency_key
    FROM confirmed_actions
    WHERE tenant_id = $2 AND idempotency_key = $4
    LIMIT 1
),
inserted_operation AS (
    INSERT INTO operation_ledger (
        operation_id,
        tenant_id,
        action_id,
        idempotency_key,
        status
    )
    SELECT $6, tenant_id, action_id, idempotency_key, 'confirmed'
    FROM canonical_action
    ON CONFLICT (tenant_id, idempotency_key) DO NOTHING
    RETURNING operation_id, action_id, idempotency_key, status, last_error
)
SELECT operation_id, action_id, idempotency_key, status, last_error
FROM inserted_operation
UNION ALL
SELECT operation_id, action_id, idempotency_key, status, last_error
FROM operation_ledger
WHERE tenant_id = $2 AND idempotency_key = $4
LIMIT 1
"#;

pub const MARK_EXECUTING: &str = r#"
UPDATE operation_ledger
SET status = 'executing',
    executing_at = $3,
    updated_at = $3,
    last_error = NULL
WHERE tenant_id = $1
  AND idempotency_key = $2
  AND status = 'confirmed'
RETURNING operation_id, action_id, idempotency_key, status, last_error
"#;

pub const MARK_SUCCEEDED: &str = r#"
UPDATE operation_ledger
SET status = 'succeeded',
    finished_at = $3,
    updated_at = $3,
    last_error = NULL
WHERE tenant_id = $1
  AND idempotency_key = $2
  AND status = 'executing'
RETURNING operation_id, action_id, idempotency_key, status, last_error
"#;

pub const MARK_FAILED: &str = r#"
UPDATE operation_ledger
SET status = 'failed',
    finished_at = $4,
    updated_at = $4,
    last_error = $3
WHERE tenant_id = $1
  AND idempotency_key = $2
  AND status = 'executing'
RETURNING operation_id, action_id, idempotency_key, status, last_error
"#;

pub const GET_BY_IDEMPOTENCY_KEY: &str = r#"
SELECT operation_id, action_id, idempotency_key, status, last_error
FROM operation_ledger
WHERE tenant_id = $1 AND idempotency_key = $2
LIMIT 1
"#;

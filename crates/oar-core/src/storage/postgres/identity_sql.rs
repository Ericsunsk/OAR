pub const TENANT_COLUMNS: &str = r#"
id,
display_name,
status
"#;

pub const WORKSPACE_USER_COLUMNS: &str = r#"
id,
tenant_id,
display_name,
status
"#;

pub const LARK_IDENTITY_COLUMNS: &str = r#"
id,
tenant_id,
actor_kind,
actor_external_id,
display_name
"#;

pub const UPSERT_TENANT: &str = r#"
INSERT INTO tenants (
    id,
    display_name,
    status
)
VALUES (
    $1,
    $2,
    $3
)
ON CONFLICT (id) DO UPDATE
SET display_name = EXCLUDED.display_name,
    status = EXCLUDED.status,
    updated_at = now()
RETURNING
id,
display_name,
status
"#;

pub const GET_TENANT_BY_ID: &str = r#"
SELECT
id,
display_name,
status
FROM tenants
WHERE id = $1
LIMIT 1
"#;

pub const UPSERT_WORKSPACE_USER: &str = r#"
WITH upserted AS (
    INSERT INTO workspace_users (
        id,
        tenant_id,
        display_name,
        status
    )
    VALUES (
        $1,
        $2,
        $3,
        $4
    )
    ON CONFLICT (id) DO UPDATE
    SET display_name = EXCLUDED.display_name,
        status = EXCLUDED.status,
        updated_at = now()
    WHERE workspace_users.tenant_id = EXCLUDED.tenant_id
    RETURNING
    id,
    tenant_id,
    display_name,
    status
)
SELECT * FROM upserted
UNION ALL
SELECT
id,
tenant_id,
display_name,
status
FROM workspace_users
WHERE id = $1
  AND tenant_id = $2
  AND NOT EXISTS (SELECT 1 FROM upserted)
"#;

pub const GET_WORKSPACE_USER_BY_ID: &str = r#"
SELECT
id,
tenant_id,
display_name,
status
FROM workspace_users
WHERE tenant_id = $1
  AND id = $2
LIMIT 1
"#;

pub const UPSERT_LARK_IDENTITY: &str = r#"
WITH upserted AS (
    INSERT INTO lark_identities (
        id,
        tenant_id,
        actor_kind,
        actor_external_id,
        display_name
    )
    VALUES (
        $1,
        $2,
        $3,
        $4,
        $5
    )
    ON CONFLICT (id) DO UPDATE
    SET actor_kind = EXCLUDED.actor_kind,
        actor_external_id = EXCLUDED.actor_external_id,
        display_name = EXCLUDED.display_name,
        updated_at = now()
    WHERE lark_identities.tenant_id = EXCLUDED.tenant_id
    RETURNING
    id,
    tenant_id,
    actor_kind,
    actor_external_id,
    display_name
)
SELECT * FROM upserted
UNION ALL
SELECT
id,
tenant_id,
actor_kind,
actor_external_id,
display_name
FROM lark_identities
WHERE id = $1
  AND tenant_id = $2
  AND NOT EXISTS (SELECT 1 FROM upserted)
"#;

pub const GET_LARK_IDENTITY_BY_ID: &str = r#"
SELECT
id,
tenant_id,
actor_kind,
actor_external_id,
display_name
FROM lark_identities
WHERE tenant_id = $1
  AND id = $2
LIMIT 1
"#;

pub const GET_LARK_IDENTITY_BY_ACTOR_EXTERNAL: &str = r#"
SELECT
id,
tenant_id,
actor_kind,
actor_external_id,
display_name
FROM lark_identities
WHERE tenant_id = $1
  AND actor_kind = $2
  AND actor_external_id = $3
LIMIT 1
"#;

use std::env;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use oar_core::domain::device_sync::{DeviceEntryPoint, DeviceSession};
use oar_core::domain::identity::{DeviceSessionId, TenantId, WorkspaceUserId};
use sqlx::postgres::PgPoolOptions;
use sqlx::{AssertSqlSafe, PgPool};

const MIGRATION_0001_SQL: &str =
    include_str!("../../../oar-core/migrations/0001_phase_0_6_identity_action_audit.sql");
const MIGRATION_0002_SQL: &str =
    include_str!("../../../oar-core/migrations/0002_review_inbox_domain.sql");
const MIGRATION_0003_SQL: &str =
    include_str!("../../../oar-core/migrations/0003_agent_model_settings.sql");

static SCHEMA_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(crate) type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

pub(crate) async fn run_live_postgres_test<F, Fut>(test_name: &str, test: F)
where
    F: FnOnce(PgPool) -> Fut,
    Fut: Future<Output = TestResult>,
{
    let Some(database_url) = env::var("DATABASE_URL").ok() else {
        eprintln!("skip {test_name}: DATABASE_URL is not set");
        return;
    };

    let schema = unique_schema_name(test_name);
    let pool = create_schema_and_pool(&database_url, &schema)
        .await
        .unwrap_or_else(|error| panic!("failed to create live postgres schema {schema}: {error}"));

    let test_result = test(pool.clone()).await;
    pool.close().await;
    let cleanup_result = drop_schema(&database_url, &schema).await;

    if let Err(error) = cleanup_result {
        panic!("failed to drop live postgres schema {schema}: {error}");
    }
    test_result.unwrap_or_else(|error| panic!("live postgres test {test_name} failed: {error}"));
}

pub(crate) async fn seed_user(
    pool: &PgPool,
    tenant_id: &str,
    user_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO tenants (id, display_name, status)
        VALUES ($1, $2, 'active')
        "#,
    )
    .bind(tenant_id)
    .bind(format!("Tenant {tenant_id}"))
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO workspace_users (id, tenant_id, display_name, status)
        VALUES ($1, $2, $3, 'active')
        "#,
    )
    .bind(user_id)
    .bind(tenant_id)
    .bind(format!("User {user_id}"))
    .execute(pool)
    .await?;

    Ok(())
}

pub(crate) fn device_session(
    tenant_id: &str,
    user_id: &str,
    session_id: &str,
    stream: &str,
    cursor: u64,
    now: SystemTime,
) -> DeviceSession {
    DeviceSession::new(
        DeviceSessionId(session_id.to_string()),
        TenantId(tenant_id.to_string()),
        WorkspaceUserId(user_id.to_string()),
        DeviceEntryPoint::MacOs,
        stream.to_string(),
        cursor,
        now,
    )
}

fn unique_schema_name(test_name: &str) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let sequence = SCHEMA_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let sanitized_name: String = test_name
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect();

    format!(
        "oar_facade_live_{}_{}_{}_{}",
        std::process::id(),
        now,
        sequence,
        sanitized_name
    )
    .to_ascii_lowercase()
}

async fn create_schema_and_pool(database_url: &str, schema: &str) -> Result<PgPool, sqlx::Error> {
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await?;

    sqlx::raw_sql(AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
        .execute(&admin_pool)
        .await?;
    sqlx::raw_sql(AssertSqlSafe(format!("SET search_path TO {schema}")))
        .execute(&admin_pool)
        .await?;
    sqlx::raw_sql(AssertSqlSafe(MIGRATION_0001_SQL.to_string()))
        .execute(&admin_pool)
        .await?;
    sqlx::raw_sql(AssertSqlSafe(MIGRATION_0002_SQL.to_string()))
        .execute(&admin_pool)
        .await?;
    sqlx::raw_sql(AssertSqlSafe(MIGRATION_0003_SQL.to_string()))
        .execute(&admin_pool)
        .await?;
    admin_pool.close().await;

    let schema_for_connection = schema.to_string();
    PgPoolOptions::new()
        .max_connections(5)
        .after_connect(move |connection, _metadata| {
            let schema = schema_for_connection.clone();
            Box::pin(async move {
                sqlx::raw_sql(AssertSqlSafe(format!("SET search_path TO {schema}")))
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect(database_url)
        .await
}

async fn drop_schema(database_url: &str, schema: &str) -> Result<(), sqlx::Error> {
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await?;
    sqlx::raw_sql(AssertSqlSafe(format!(
        "DROP SCHEMA IF EXISTS {schema} CASCADE"
    )))
    .execute(&admin_pool)
    .await?;
    admin_pool.close().await;
    Ok(())
}

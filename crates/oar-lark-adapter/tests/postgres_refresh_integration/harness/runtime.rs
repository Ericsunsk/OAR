use std::env;
use std::future::Future;
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::postgres::PgPoolOptions;
use sqlx::{AssertSqlSafe, PgPool};

use super::constants::{MIGRATION_0001_SQL, MIGRATION_0002_SQL};

pub(crate) type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

pub(crate) fn run_live_postgres_test<F, Fut>(test_name: &str, test: F)
where
    F: FnOnce(PgPool) -> Fut,
    Fut: Future<Output = TestResult>,
{
    let Some(database_url) = env::var("DATABASE_URL").ok() else {
        eprintln!("skip {test_name}: DATABASE_URL is not set");
        return;
    };

    runtime().block_on(async move {
        let schema = unique_schema_name(test_name);
        let pool = create_schema_and_pool(&database_url, &schema)
            .await
            .unwrap_or_else(|error| {
                panic!("failed to create live postgres schema {schema}: {error}")
            });

        let test_result = test(pool.clone()).await;
        pool.close().await;
        let cleanup_result = drop_schema(&database_url, &schema).await;

        if let Err(error) = cleanup_result {
            panic!("failed to drop live postgres schema {schema}: {error}");
        }
        test_result
            .unwrap_or_else(|error| panic!("live postgres test {test_name} failed: {error}"));
    });
}

pub(crate) fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime should build")
}

pub(crate) fn unique_schema_name(test_name: &str) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let sanitized_name: String = test_name
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect();
    format!(
        "oar_lark_adapter_live_{}_{}_{}",
        std::process::id(),
        now,
        sanitized_name
    )
    .to_ascii_lowercase()
}

pub(crate) async fn create_schema_and_pool(
    database_url: &str,
    schema: &str,
) -> Result<PgPool, sqlx::Error> {
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

pub(crate) async fn drop_schema(database_url: &str, schema: &str) -> Result<(), sqlx::Error> {
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

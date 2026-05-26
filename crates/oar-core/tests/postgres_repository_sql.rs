#[path = "postgres_repository_sql/audit_sql.rs"]
mod audit_sql;
#[path = "postgres_repository_sql/device_session_sql.rs"]
mod device_session_sql;
#[path = "postgres_repository_sql/identity_sql.rs"]
mod identity_sql;
#[path = "postgres_repository_sql/ledger_sql.rs"]
mod ledger_sql;
#[path = "postgres_repository_sql/token_grant_sql.rs"]
mod token_grant_sql;

fn compact(sql: &str) -> String {
    sql.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

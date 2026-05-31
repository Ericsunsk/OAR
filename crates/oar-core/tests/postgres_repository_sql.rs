#[path = "postgres_repository_sql/audit_sql.rs"]
mod audit_sql;
#[path = "postgres_repository_sql/device_session_sql.rs"]
mod device_session_sql;
#[path = "postgres_repository_sql/identity_sql.rs"]
mod identity_sql;
#[path = "postgres_repository_sql/ledger_sql.rs"]
mod ledger_sql;
#[path = "postgres_repository_sql/operational_recovery_sql.rs"]
mod operational_recovery_sql;
#[path = "postgres_repository_sql/review_inbox_sql.rs"]
mod review_inbox_sql;
#[path = "postgres_repository_sql/scheduler_sql.rs"]
mod scheduler_sql;
#[path = "postgres_repository_sql/token_grant_sql.rs"]
mod token_grant_sql;

fn compact(sql: &str) -> String {
    sql.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

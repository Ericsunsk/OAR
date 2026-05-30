use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

fn migration_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations")
}

fn read_all_migration_sql() -> String {
    let dir = migration_dir();
    assert!(
        dir.exists(),
        "expected migration directory at {}",
        dir.display()
    );

    let mut sql_files = Vec::new();
    for entry in fs::read_dir(&dir).expect("failed to read migration directory") {
        let entry = entry.expect("failed to read migration directory entry");
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("sql") {
            let content = fs::read_to_string(&path)
                .unwrap_or_else(|_| panic!("failed to read {}", path.display()));
            sql_files.push(content);
        }
    }

    assert!(
        !sql_files.is_empty(),
        "expected at least one .sql migration file in {}",
        dir.display()
    );
    sql_files.join("\n")
}

fn all_sql_lowercase() -> &'static str {
    static SQL: OnceLock<String> = OnceLock::new();
    SQL.get_or_init(|| read_all_migration_sql().to_lowercase())
}

#[path = "postgres_schema_contract/action_ledger.rs"]
mod action_ledger;
#[path = "postgres_schema_contract/audit.rs"]
mod audit;
#[path = "postgres_schema_contract/identity_tokens_devices.rs"]
mod identity_tokens_devices;
#[path = "postgres_schema_contract/review_inbox.rs"]
mod review_inbox;
#[path = "postgres_schema_contract/scheduler.rs"]
mod scheduler;

use super::*;

use std::collections::HashSet;

#[test]
fn postgres_live_evidence_schema_excludes_raw_content_and_tokens() {
    run_live_postgres_test("review_inbox_evidence_schema_guard", |pool| async move {
        let rows = sqlx::query(
            "SELECT column_name FROM information_schema.columns WHERE table_schema = current_schema() AND table_name = 'evidence_items'",
        )
        .fetch_all(&pool)
        .await?;

        let names: HashSet<String> = rows
            .into_iter()
            .map(|row| row.try_get::<String, _>("column_name"))
            .collect::<Result<_, _>>()?;

        for forbidden in [
            "raw_content",
            "raw_transcript",
            "access_token",
            "refresh_token",
        ] {
            assert!(!names.contains(forbidden));
        }

        Ok(())
    });
}

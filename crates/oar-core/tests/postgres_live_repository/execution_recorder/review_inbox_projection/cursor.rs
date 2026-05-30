use super::*;

#[test]
fn postgres_live_execution_recorder_projection_keeps_source_cursor_and_allows_new_source_sync() {
    run_live_postgres_test(
        "execution_recorder_review_inbox_source_cursor",
        |pool| async move {
            seed_user(
                &pool,
                "tenant_recorder_source_cursor",
                "user_recorder_source_cursor",
            )
            .await?;

            let recorder = PostgresExecutionRecorder::new(pool.clone());
            let repository = PostgresReviewInboxRepository::new(pool.clone());
            let action = confirmed_action(
                "action_recorder_source_cursor",
                "tenant_recorder_source_cursor",
                "user_recorder_source_cursor",
                "idem_recorder_source_cursor",
            );

            recorder
                .record_confirmation(
                    &action,
                    1_748_250_000_000,
                    "op_recorder_source_cursor",
                    &AuditEvent::confirmed_action(
                        audit_context(
                            "evt_recorder_source_cursor_1",
                            "trace_recorder_source_cursor",
                            1,
                            1_748_250_001_000,
                            "user_recorder_source_cursor",
                            "tenant_recorder_source_cursor",
                            "action_recorder_source_cursor",
                        ),
                        summary("confirmed"),
                    ),
                    &outbox_envelope(
                        "tenant_recorder_source_cursor",
                        "trace_recorder_source_cursor",
                        1_748_250_010_000,
                    ),
                )
                .await?;
            seed_confirmed_inbox_projection(
                &pool,
                "tenant_recorder_source_cursor",
                "user_recorder_source_cursor",
                "proposed_recorder_source_cursor",
                "inbox_recorder_source_cursor",
                "op_recorder_source_cursor",
                100,
            )
            .await?;

            let projection_now_ms = 1_748_250_002_000;
            let dry_run = recorder
                .record_dry_run(
                    "tenant_recorder_source_cursor",
                    "idem_recorder_source_cursor",
                    projection_now_ms,
                    &AuditEvent::dry_run(
                        audit_context(
                            "evt_recorder_source_cursor_2",
                            "trace_recorder_source_cursor",
                            2,
                            projection_now_ms,
                            "user_recorder_source_cursor",
                            "tenant_recorder_source_cursor",
                            "action_recorder_source_cursor",
                        ),
                        Some(summary("before")),
                        Some(summary("projected")),
                    ),
                    &outbox_envelope(
                        "tenant_recorder_source_cursor",
                        "trace_recorder_source_cursor",
                        1_748_250_011_000,
                    ),
                )
                .await?;
            assert_eq!(
                dry_run.inbox_item_id.as_deref(),
                Some("inbox_recorder_source_cursor")
            );

            let projected_row = sqlx::query(
                r#"
                SELECT source_cursor_value, sync_cursor_value
                FROM review_inbox_items
                WHERE tenant_id = $1 AND id = $2
                "#,
            )
            .bind("tenant_recorder_source_cursor")
            .bind("inbox_recorder_source_cursor")
            .fetch_one(&pool)
            .await?;
            let source_cursor_value: i64 = projected_row.try_get("source_cursor_value")?;
            let projected_sync_cursor_value: i64 = projected_row.try_get("sync_cursor_value")?;
            assert_eq!(source_cursor_value, 100);
            assert_eq!(projected_sync_cursor_value, 101);

            let upserted = repository
                .upsert_review_inbox_item(&execution_projection_inbox_item(ProjectionInboxSpec {
                    id: "inbox_recorder_source_cursor",
                    tenant_id: "tenant_recorder_source_cursor",
                    user_id: "user_recorder_source_cursor",
                    proposed_action_id: "proposed_recorder_source_cursor",
                    status: ReviewInboxItemStatus::Confirmed,
                    ledger_status: Some("confirmed"),
                    operation_id: Some("op_recorder_source_cursor"),
                    sync_cursor: 101,
                }))
                .await?;
            assert_eq!(upserted.as_deref(), Some("inbox_recorder_source_cursor"));

            let after_source_sync = sqlx::query(
                r#"
                SELECT source_cursor_value, sync_cursor_value
                FROM review_inbox_items
                WHERE tenant_id = $1 AND id = $2
                "#,
            )
            .bind("tenant_recorder_source_cursor")
            .bind("inbox_recorder_source_cursor")
            .fetch_one(&pool)
            .await?;
            let next_source_cursor_value: i64 = after_source_sync.try_get("source_cursor_value")?;
            let next_sync_cursor_value: i64 = after_source_sync.try_get("sync_cursor_value")?;
            assert_eq!(next_source_cursor_value, 101);
            assert_eq!(next_sync_cursor_value, 102);

            Ok(())
        },
    );
}

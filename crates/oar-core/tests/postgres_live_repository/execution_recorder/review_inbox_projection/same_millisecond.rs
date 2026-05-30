use super::*;

#[test]
fn postgres_live_execution_recorder_advances_inbox_projection_with_same_millisecond_events() {
    run_live_postgres_test(
        "execution_recorder_review_inbox_same_ms",
        |pool| async move {
            seed_user(&pool, "tenant_recorder_same_ms", "user_recorder_same_ms").await?;

            let recorder = PostgresExecutionRecorder::new(pool.clone());
            let repository = PostgresReviewInboxRepository::new(pool.clone());
            let action = confirmed_action(
                "action_recorder_same_ms",
                "tenant_recorder_same_ms",
                "user_recorder_same_ms",
                "idem_recorder_same_ms",
            );

            recorder
                .record_confirmation(
                    &action,
                    1_748_250_000_000,
                    "op_recorder_same_ms",
                    &AuditEvent::confirmed_action(
                        audit_context(
                            "evt_recorder_same_ms_1",
                            "trace_recorder_same_ms",
                            1,
                            1_748_250_001_000,
                            "user_recorder_same_ms",
                            "tenant_recorder_same_ms",
                            "action_recorder_same_ms",
                        ),
                        summary("confirmed"),
                    ),
                    &outbox_envelope(
                        "tenant_recorder_same_ms",
                        "trace_recorder_same_ms",
                        1_748_250_010_000,
                    ),
                )
                .await?;
            seed_confirmed_inbox_projection(
                &pool,
                "tenant_recorder_same_ms",
                "user_recorder_same_ms",
                "proposed_recorder_same_ms",
                "inbox_recorder_same_ms",
                "op_recorder_same_ms",
                1_748_250_002_000,
            )
            .await?;

            let same_ms = 1_748_250_003_000;
            let dry_run = recorder
                .record_dry_run(
                    "tenant_recorder_same_ms",
                    "idem_recorder_same_ms",
                    same_ms,
                    &AuditEvent::dry_run(
                        audit_context(
                            "evt_recorder_same_ms_2",
                            "trace_recorder_same_ms",
                            2,
                            same_ms,
                            "user_recorder_same_ms",
                            "tenant_recorder_same_ms",
                            "action_recorder_same_ms",
                        ),
                        Some(summary("before")),
                        Some(summary("projected")),
                    ),
                    &outbox_envelope("tenant_recorder_same_ms", "trace_recorder_same_ms", same_ms),
                )
                .await?;
            assert_eq!(
                dry_run.inbox_item_id.as_deref(),
                Some("inbox_recorder_same_ms")
            );
            let after_dry_run = repository
                .list_review_inbox_items("tenant_recorder_same_ms", "user_recorder_same_ms", 0, 10)
                .await?;
            assert_eq!(after_dry_run[0].status, ReviewInboxItemStatus::Executing);
            let executing_cursor = after_dry_run[0].sync_cursor_value;

            let success = recorder
                .record_success(
                    "tenant_recorder_same_ms",
                    "idem_recorder_same_ms",
                    same_ms,
                    &AuditEvent::execution_succeeded(
                        audit_context(
                            "evt_recorder_same_ms_3",
                            "trace_recorder_same_ms",
                            3,
                            same_ms,
                            "user_recorder_same_ms",
                            "tenant_recorder_same_ms",
                            "action_recorder_same_ms",
                        ),
                        Some(summary("before")),
                        Some(summary("applied")),
                        "lark_op_same_ms",
                    ),
                    &outbox_envelope("tenant_recorder_same_ms", "trace_recorder_same_ms", same_ms),
                )
                .await?;
            assert_eq!(
                success.inbox_item_id.as_deref(),
                Some("inbox_recorder_same_ms")
            );

            let after_success = repository
                .list_review_inbox_items("tenant_recorder_same_ms", "user_recorder_same_ms", 0, 10)
                .await?;
            assert_eq!(after_success[0].status, ReviewInboxItemStatus::Succeeded);
            assert_eq!(
                after_success[0].ledger_status,
                Some(ActionStatus::Succeeded)
            );
            assert!(
                after_success[0].sync_cursor_value > executing_cursor,
                "DB-owned sync cursor must advance even when ledger events share a millisecond"
            );

            Ok(())
        },
    );
}

use super::*;

#[test]
fn postgres_live_execution_recorder_projects_ledger_status_to_review_inbox() {
    run_live_postgres_test(
        "execution_recorder_review_inbox_projection",
        |pool| async move {
            seed_user(
                &pool,
                "tenant_recorder_projection",
                "user_recorder_projection",
            )
            .await?;

            let recorder = PostgresExecutionRecorder::new(pool.clone());
            let repository = PostgresReviewInboxRepository::new(pool.clone());
            let action = confirmed_action(
                "action_recorder_projection",
                "tenant_recorder_projection",
                "user_recorder_projection",
                "idem_recorder_projection",
            );

            recorder
                .record_confirmation(
                    &action,
                    1_748_250_000_000,
                    "op_recorder_projection",
                    &AuditEvent::confirmed_action(
                        audit_context(
                            "evt_recorder_projection_1",
                            "trace_recorder_projection",
                            1,
                            1_748_250_001_000,
                            "user_recorder_projection",
                            "tenant_recorder_projection",
                            "action_recorder_projection",
                        ),
                        summary("confirmed"),
                    ),
                    &outbox_envelope(
                        "tenant_recorder_projection",
                        "trace_recorder_projection",
                        1_748_250_010_000,
                    ),
                )
                .await?;
            seed_confirmed_inbox_projection(
                &pool,
                "tenant_recorder_projection",
                "user_recorder_projection",
                "proposed_recorder_projection",
                "inbox_recorder_projection",
                "op_recorder_projection",
                100,
            )
            .await?;

            let dry_run = recorder
                .record_dry_run(
                    "tenant_recorder_projection",
                    "idem_recorder_projection",
                    1_748_250_002_000,
                    &AuditEvent::dry_run(
                        audit_context(
                            "evt_recorder_projection_2",
                            "trace_recorder_projection",
                            2,
                            1_748_250_002_000,
                            "user_recorder_projection",
                            "tenant_recorder_projection",
                            "action_recorder_projection",
                        ),
                        Some(summary("before")),
                        Some(summary("projected")),
                    ),
                    &outbox_envelope(
                        "tenant_recorder_projection",
                        "trace_recorder_projection",
                        1_748_250_011_000,
                    ),
                )
                .await?;
            assert_eq!(
                dry_run.inbox_item_id.as_deref(),
                Some("inbox_recorder_projection")
            );

            let after_dry_run = repository
                .list_review_inbox_items(
                    "tenant_recorder_projection",
                    "user_recorder_projection",
                    0,
                    10,
                )
                .await?;
            assert_eq!(after_dry_run[0].status, ReviewInboxItemStatus::Executing);
            assert_eq!(
                after_dry_run[0].ledger_status,
                Some(ActionStatus::Executing)
            );
            assert_eq!(after_dry_run[0].sync_cursor_value, 101);

            let success = recorder
                .record_success(
                    "tenant_recorder_projection",
                    "idem_recorder_projection",
                    1_748_250_003_000,
                    &AuditEvent::execution_succeeded(
                        audit_context(
                            "evt_recorder_projection_3",
                            "trace_recorder_projection",
                            3,
                            1_748_250_003_000,
                            "user_recorder_projection",
                            "tenant_recorder_projection",
                            "action_recorder_projection",
                        ),
                        Some(summary("before")),
                        Some(summary("applied")),
                        "lark_op_projection",
                    ),
                    &outbox_envelope(
                        "tenant_recorder_projection",
                        "trace_recorder_projection",
                        1_748_250_012_000,
                    ),
                )
                .await?;
            assert_eq!(
                success.inbox_item_id.as_deref(),
                Some("inbox_recorder_projection")
            );

            let after_success = repository
                .list_review_inbox_items(
                    "tenant_recorder_projection",
                    "user_recorder_projection",
                    0,
                    10,
                )
                .await?;
            assert_eq!(after_success[0].status, ReviewInboxItemStatus::Succeeded);
            assert_eq!(
                after_success[0].ledger_status,
                Some(ActionStatus::Succeeded)
            );
            assert_eq!(after_success[0].sync_cursor_value, 102);

            let duplicate_success = recorder
                .record_success(
                    "tenant_recorder_projection",
                    "idem_recorder_projection",
                    1_748_250_004_000,
                    &AuditEvent::execution_succeeded(
                        audit_context(
                            "evt_recorder_projection_4",
                            "trace_recorder_projection",
                            4,
                            1_748_250_004_000,
                            "user_recorder_projection",
                            "tenant_recorder_projection",
                            "action_recorder_projection",
                        ),
                        Some(summary("before")),
                        Some(summary("applied again")),
                        "lark_op_projection_retry",
                    ),
                    &outbox_envelope(
                        "tenant_recorder_projection",
                        "trace_recorder_projection",
                        1_748_250_013_000,
                    ),
                )
                .await?;
            assert!(duplicate_success.duplicate);
            assert_eq!(duplicate_success.inbox_item_id, None);

            let after_duplicate = repository
                .list_review_inbox_items(
                    "tenant_recorder_projection",
                    "user_recorder_projection",
                    0,
                    10,
                )
                .await?;
            assert_eq!(after_duplicate[0].status, ReviewInboxItemStatus::Succeeded);
            assert_eq!(after_duplicate[0].sync_cursor_value, 102);

            Ok(())
        },
    );
}

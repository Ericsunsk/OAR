use super::super::harness::*;
use super::support::{
    execution_projection_inbox_item, seed_confirmed_inbox_projection, ProjectionInboxSpec,
};

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

#[test]
fn postgres_live_execution_recorder_projects_failure_to_review_inbox() {
    run_live_postgres_test(
        "execution_recorder_review_inbox_failure",
        |pool| async move {
            seed_user(
                &pool,
                "tenant_recorder_projection_failure",
                "user_recorder_projection_failure",
            )
            .await?;

            let recorder = PostgresExecutionRecorder::new(pool.clone());
            let repository = PostgresReviewInboxRepository::new(pool.clone());
            let action = confirmed_action(
                "action_recorder_projection_failure",
                "tenant_recorder_projection_failure",
                "user_recorder_projection_failure",
                "idem_recorder_projection_failure",
            );

            recorder
                .record_confirmation(
                    &action,
                    1_748_250_000_000,
                    "op_recorder_projection_failure",
                    &AuditEvent::confirmed_action(
                        audit_context(
                            "evt_recorder_projection_failure_1",
                            "trace_recorder_projection_failure",
                            1,
                            1_748_250_001_000,
                            "user_recorder_projection_failure",
                            "tenant_recorder_projection_failure",
                            "action_recorder_projection_failure",
                        ),
                        summary("confirmed"),
                    ),
                    &outbox_envelope(
                        "tenant_recorder_projection_failure",
                        "trace_recorder_projection_failure",
                        1_748_250_010_000,
                    ),
                )
                .await?;
            seed_confirmed_inbox_projection(
                &pool,
                "tenant_recorder_projection_failure",
                "user_recorder_projection_failure",
                "proposed_recorder_projection_failure",
                "inbox_recorder_projection_failure",
                "op_recorder_projection_failure",
                100,
            )
            .await?;
            recorder
                .record_dry_run(
                    "tenant_recorder_projection_failure",
                    "idem_recorder_projection_failure",
                    1_748_250_002_000,
                    &AuditEvent::dry_run(
                        audit_context(
                            "evt_recorder_projection_failure_2",
                            "trace_recorder_projection_failure",
                            2,
                            1_748_250_002_000,
                            "user_recorder_projection_failure",
                            "tenant_recorder_projection_failure",
                            "action_recorder_projection_failure",
                        ),
                        Some(summary("before")),
                        Some(summary("projected")),
                    ),
                    &outbox_envelope(
                        "tenant_recorder_projection_failure",
                        "trace_recorder_projection_failure",
                        1_748_250_011_000,
                    ),
                )
                .await?;

            let failed = recorder
                .record_failure(
                    "tenant_recorder_projection_failure",
                    "idem_recorder_projection_failure",
                    "adapter timeout",
                    1_748_250_003_000,
                    &AuditEvent::execution_failed(
                        audit_context(
                            "evt_recorder_projection_failure_3",
                            "trace_recorder_projection_failure",
                            3,
                            1_748_250_003_000,
                            "user_recorder_projection_failure",
                            "tenant_recorder_projection_failure",
                            "action_recorder_projection_failure",
                        ),
                        Some(summary("before")),
                        None,
                        "adapter_timeout",
                        "adapter timeout",
                    ),
                    &outbox_envelope(
                        "tenant_recorder_projection_failure",
                        "trace_recorder_projection_failure",
                        1_748_250_012_000,
                    ),
                )
                .await?;
            assert_eq!(
                failed.inbox_item_id.as_deref(),
                Some("inbox_recorder_projection_failure")
            );

            let rows = repository
                .list_review_inbox_items(
                    "tenant_recorder_projection_failure",
                    "user_recorder_projection_failure",
                    0,
                    10,
                )
                .await?;
            assert_eq!(rows[0].status, ReviewInboxItemStatus::Failed);
            assert_eq!(rows[0].ledger_status, Some(ActionStatus::Failed));

            Ok(())
        },
    );
}

#[test]
fn postgres_live_execution_recorder_does_not_overwrite_terminal_inbox_projection() {
    run_live_postgres_test(
        "execution_recorder_review_inbox_terminal_guard",
        |pool| async move {
            seed_user(
                &pool,
                "tenant_recorder_projection_terminal",
                "user_recorder_projection_terminal",
            )
            .await?;

            let recorder = PostgresExecutionRecorder::new(pool.clone());
            let repository = PostgresReviewInboxRepository::new(pool.clone());
            let action = confirmed_action(
                "action_recorder_projection_terminal",
                "tenant_recorder_projection_terminal",
                "user_recorder_projection_terminal",
                "idem_recorder_projection_terminal",
            );

            recorder
                .record_confirmation(
                    &action,
                    1_748_250_000_000,
                    "op_recorder_projection_terminal",
                    &AuditEvent::confirmed_action(
                        audit_context(
                            "evt_recorder_projection_terminal_1",
                            "trace_recorder_projection_terminal",
                            1,
                            1_748_250_001_000,
                            "user_recorder_projection_terminal",
                            "tenant_recorder_projection_terminal",
                            "action_recorder_projection_terminal",
                        ),
                        summary("confirmed"),
                    ),
                    &outbox_envelope(
                        "tenant_recorder_projection_terminal",
                        "trace_recorder_projection_terminal",
                        1_748_250_010_000,
                    ),
                )
                .await?;
            seed_confirmed_inbox_projection(
                &pool,
                "tenant_recorder_projection_terminal",
                "user_recorder_projection_terminal",
                "proposed_recorder_projection_terminal",
                "inbox_recorder_projection_terminal",
                "op_recorder_projection_terminal",
                100,
            )
            .await?;
            repository
                .upsert_review_inbox_item(&execution_projection_inbox_item(ProjectionInboxSpec {
                    id: "inbox_recorder_projection_terminal",
                    tenant_id: "tenant_recorder_projection_terminal",
                    user_id: "user_recorder_projection_terminal",
                    proposed_action_id: "proposed_recorder_projection_terminal",
                    status: ReviewInboxItemStatus::Rejected,
                    ledger_status: Some("confirmed"),
                    operation_id: Some("op_recorder_projection_terminal"),
                    sync_cursor: 200,
                }))
                .await?;

            let dry_run = recorder
                .record_dry_run(
                    "tenant_recorder_projection_terminal",
                    "idem_recorder_projection_terminal",
                    1_748_250_002_000,
                    &AuditEvent::dry_run(
                        audit_context(
                            "evt_recorder_projection_terminal_2",
                            "trace_recorder_projection_terminal",
                            2,
                            1_748_250_002_000,
                            "user_recorder_projection_terminal",
                            "tenant_recorder_projection_terminal",
                            "action_recorder_projection_terminal",
                        ),
                        Some(summary("before")),
                        Some(summary("projected")),
                    ),
                    &outbox_envelope(
                        "tenant_recorder_projection_terminal",
                        "trace_recorder_projection_terminal",
                        1_748_250_011_000,
                    ),
                )
                .await?;
            assert_eq!(dry_run.operation.status, ActionStatus::Executing);
            assert_eq!(dry_run.inbox_item_id, None);

            let rows = repository
                .list_review_inbox_items(
                    "tenant_recorder_projection_terminal",
                    "user_recorder_projection_terminal",
                    0,
                    10,
                )
                .await?;
            assert_eq!(rows[0].status, ReviewInboxItemStatus::Rejected);
            assert_eq!(rows[0].ledger_status, Some(ActionStatus::Confirmed));
            assert_eq!(rows[0].sync_cursor_value, 200);

            Ok(())
        },
    );
}

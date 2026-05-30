use super::*;

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

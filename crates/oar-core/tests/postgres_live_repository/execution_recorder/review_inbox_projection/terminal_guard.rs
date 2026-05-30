use super::*;

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

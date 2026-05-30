use super::*;

#[test]
fn postgres_live_review_decision_recorder_confirm_and_reject() {
    run_live_postgres_test(
        "review_decision_recorder_confirm_reject",
        |pool| async move {
            seed_user(&pool, "tenant_recorder", "user_recorder").await?;
            let repository = PostgresReviewInboxRepository::new(pool.clone());
            let recorder = PostgresReviewDecisionRecorder::new(pool.clone());

            let action = proposed_action("tenant_recorder", "user_recorder", "action_recorder", 1);
            repository
                .insert_proposed_action(&action, Some(ms(1_748_250_020_000)))
                .await?;
            repository
                .upsert_review_inbox_item(&inbox_item(InboxItemSpec {
                    id: "inbox_recorder",
                    tenant_id: "tenant_recorder",
                    user_id: "user_recorder",
                    proposed_action_id: "action_recorder",
                    proposed_action_version: 1,
                    sort_key: 500,
                    sync_cursor: 300,
                    status: ReviewInboxItemStatus::Open,
                    ledger_status: None,
                    operation_id: None,
                }))
                .await?;

            let report = recorder
                .record_decision(PostgresReviewDecisionRecorderRequest {
                    decision: InsertProposedActionDecisionRequest {
                        id: "decision_recorder_reject",
                        tenant_id: "tenant_recorder",
                        proposed_action_id: "action_recorder",
                        proposed_action_version: 1,
                        actor_user_id: "user_recorder",
                        decision: &ProposedActionDecision::Reject,
                        confirmed_action_id: None,
                        decided_at: ms(1_748_250_021_000),
                    },
                    confirmed_action: None,
                    confirmed_at_ms: None,
                    operation_id: None,
                    inbox_item: &inbox_item(InboxItemSpec {
                        id: "inbox_recorder",
                        tenant_id: "tenant_recorder",
                        user_id: "user_recorder",
                        proposed_action_id: "action_recorder",
                        proposed_action_version: 1,
                        sort_key: 500,
                        sync_cursor: 301,
                        status: ReviewInboxItemStatus::Rejected,
                        ledger_status: None,
                        operation_id: None,
                    }),
                    event: &AuditEvent::proposed_action_decision(
                        audit_context(
                            "evt_recorder_reject",
                            "trace_recorder",
                            1,
                            1_748_250_021_000,
                            "user_recorder",
                            "tenant_recorder",
                            "action_recorder",
                        ),
                        summary("reject decision"),
                    ),
                    outbox: &outbox_envelope(
                        "tenant_recorder",
                        "trace_recorder",
                        1_748_250_022_000,
                    ),
                })
                .await?;
            assert!(!report.duplicate);
            assert_eq!(report.operation, None);

            Ok(())
        },
    );
}

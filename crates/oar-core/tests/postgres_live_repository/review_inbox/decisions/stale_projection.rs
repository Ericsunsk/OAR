use super::super::*;

#[test]
fn postgres_live_review_decision_recorder_rolls_back_on_stale_inbox_projection() {
    run_live_postgres_test("review_decision_recorder_stale_inbox", |pool| async move {
        seed_user(&pool, "tenant_recorder_stale", "user_recorder_stale").await?;
        let repository = PostgresReviewInboxRepository::new(pool.clone());
        let recorder = PostgresReviewDecisionRecorder::new(pool.clone());

        let action = proposed_action(
            "tenant_recorder_stale",
            "user_recorder_stale",
            "action_recorder_stale",
            1,
        );
        repository
            .insert_proposed_action(&action, Some(ms(1_748_250_025_000)))
            .await?;
        repository
            .upsert_review_inbox_item(&inbox_item(InboxItemSpec {
                id: "inbox_recorder_stale",
                tenant_id: "tenant_recorder_stale",
                user_id: "user_recorder_stale",
                proposed_action_id: "action_recorder_stale",
                proposed_action_version: 1,
                sort_key: 550,
                sync_cursor: 900,
                status: ReviewInboxItemStatus::Open,
                ledger_status: None,
                operation_id: None,
            }))
            .await?;

        let error = recorder
            .record_decision(PostgresReviewDecisionRecorderRequest {
                expected_sync_cursor_value: 899,
                decision: InsertProposedActionDecisionRequest {
                    id: "decision_recorder_stale",
                    tenant_id: "tenant_recorder_stale",
                    proposed_action_id: "action_recorder_stale",
                    proposed_action_version: 1,
                    actor_user_id: "user_recorder_stale",
                    decision: &ProposedActionDecision::Reject,
                    confirmed_action_id: None,
                    decided_at: ms(1_748_250_026_000),
                },
                confirmed_action: None,
                confirmed_at_ms: None,
                operation_id: None,
                inbox_item: &inbox_item(InboxItemSpec {
                    id: "inbox_recorder_stale",
                    tenant_id: "tenant_recorder_stale",
                    user_id: "user_recorder_stale",
                    proposed_action_id: "action_recorder_stale",
                    proposed_action_version: 1,
                    sort_key: 550,
                    sync_cursor: 900,
                    status: ReviewInboxItemStatus::Rejected,
                    ledger_status: None,
                    operation_id: None,
                }),
                event: &AuditEvent::proposed_action_decision(
                    audit_context(
                        "evt_recorder_stale",
                        "trace_recorder_stale",
                        1,
                        1_748_250_026_000,
                        "user_recorder_stale",
                        "tenant_recorder_stale",
                        "action_recorder_stale",
                    ),
                    summary("stale reject decision"),
                ),
                outbox: &outbox_envelope(
                    "tenant_recorder_stale",
                    "trace_recorder_stale",
                    1_748_250_027_000,
                ),
            })
            .await
            .expect_err("stale inbox update should roll back decision transaction");

        assert!(matches!(
            error,
            PostgresRepositoryError::ReviewDecisionRequestMismatch {
                field: "inbox_item.sync_cursor",
                ..
            }
        ));

        let decision_count: i64 = sqlx::query_scalar(
            r#"
            SELECT count(*)
            FROM proposed_action_decisions
            WHERE tenant_id = $1 AND id = $2
            "#,
        )
        .bind("tenant_recorder_stale")
        .bind("decision_recorder_stale")
        .fetch_one(&pool)
        .await?;
        assert_eq!(decision_count, 0);

        let outbox_count: i64 = sqlx::query_scalar(
            r#"
            SELECT count(*)
            FROM audit_outbox
            WHERE tenant_id = $1 AND aggregate_id = $2
            "#,
        )
        .bind("tenant_recorder_stale")
        .bind("trace_recorder_stale")
        .fetch_one(&pool)
        .await?;
        assert_eq!(outbox_count, 0);

        Ok(())
    });
}

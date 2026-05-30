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
                    expected_sync_cursor_value: 300,
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

            let confirm_action = proposed_action(
                "tenant_recorder",
                "user_recorder",
                "action_recorder_confirm",
                1,
            );
            repository
                .insert_proposed_action(&confirm_action, Some(ms(1_748_250_023_000)))
                .await?;
            repository
                .upsert_review_inbox_item(&inbox_item(InboxItemSpec {
                    id: "inbox_recorder_confirm",
                    tenant_id: "tenant_recorder",
                    user_id: "user_recorder",
                    proposed_action_id: "action_recorder_confirm",
                    proposed_action_version: 1,
                    sort_key: 501,
                    sync_cursor: 400,
                    status: ReviewInboxItemStatus::Open,
                    ledger_status: None,
                    operation_id: None,
                }))
                .await?;
            let mut confirm_action_for_decision = confirm_action.clone();
            let confirmed_action = confirm_action_for_decision
                .decide(ProposedActionDecision::Confirm, ms(1_748_250_024_000))
                .expect("confirm decision should be valid")
                .expect("confirm decision should create confirmed action");
            let operation_id = format!("op-{}", confirmed_action.idempotency_key);
            let report = recorder
                .record_decision(PostgresReviewDecisionRecorderRequest {
                    expected_sync_cursor_value: 400,
                    decision: InsertProposedActionDecisionRequest {
                        id: "decision_recorder_confirm",
                        tenant_id: "tenant_recorder",
                        proposed_action_id: "action_recorder_confirm",
                        proposed_action_version: 1,
                        actor_user_id: "user_recorder",
                        decision: &ProposedActionDecision::Confirm,
                        confirmed_action_id: Some(&confirmed_action.action_id),
                        decided_at: ms(1_748_250_024_000),
                    },
                    confirmed_action: Some(&confirmed_action),
                    confirmed_at_ms: Some(1_748_250_024_000),
                    operation_id: Some(&operation_id),
                    inbox_item: &inbox_item(InboxItemSpec {
                        id: "inbox_recorder_confirm",
                        tenant_id: "tenant_recorder",
                        user_id: "user_recorder",
                        proposed_action_id: "action_recorder_confirm",
                        proposed_action_version: 1,
                        sort_key: 501,
                        sync_cursor: 401,
                        status: ReviewInboxItemStatus::Confirmed,
                        ledger_status: Some("confirmed"),
                        operation_id: Some(&operation_id),
                    }),
                    event: &AuditEvent::proposed_action_decision(
                        audit_context(
                            "evt_recorder_confirm",
                            "trace_recorder_confirm",
                            1,
                            1_748_250_024_000,
                            "user_recorder",
                            "tenant_recorder",
                            "action_recorder_confirm",
                        ),
                        summary("confirm decision"),
                    ),
                    outbox: &outbox_envelope(
                        "tenant_recorder",
                        "trace_recorder_confirm",
                        1_748_250_024_500,
                    ),
                })
                .await?;
            assert!(!report.duplicate);
            assert_eq!(
                report.inbox_item_id.as_deref(),
                Some("inbox_recorder_confirm")
            );
            assert_eq!(
                report.operation.as_ref().map(|operation| operation.status),
                Some(ActionStatus::Confirmed)
            );

            let projected = sqlx::query(
                r#"
                SELECT status, ledger_status, operation_id
                FROM review_inbox_items
                WHERE tenant_id = $1 AND id = $2
                "#,
            )
            .bind("tenant_recorder")
            .bind("inbox_recorder_confirm")
            .fetch_one(&pool)
            .await?;
            assert_eq!(projected.try_get::<String, _>("status")?, "confirmed");
            assert_eq!(
                projected.try_get::<Option<String>, _>("ledger_status")?,
                Some("confirmed".to_string())
            );
            assert_eq!(
                projected.try_get::<Option<String>, _>("operation_id")?,
                Some(operation_id)
            );

            Ok(())
        },
    );
}

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

#[test]
fn postgres_live_review_decision_context_is_exactly_scoped() {
    run_live_postgres_test("review_decision_context_scope", |pool| async move {
        seed_user(&pool, "tenant_ctx", "user_ctx").await?;
        seed_user(&pool, "tenant_ctx_other", "user_ctx").await?;
        sqlx::query(
            r#"
            INSERT INTO workspace_users (id, tenant_id, display_name, status)
            VALUES ($1, $2, $3, 'active')
            "#,
        )
        .bind("user_ctx_other")
        .bind("tenant_ctx")
        .bind("User user_ctx_other")
        .execute(&pool)
        .await?;
        let repository = PostgresReviewInboxRepository::new(pool.clone());
        let recorder = PostgresReviewDecisionRecorder::new(pool.clone());

        repository
            .insert_evidence_item(
                "tenant_ctx",
                &evidence_item("evidence_ctx", "Decision context evidence", "kr_ctx"),
            )
            .await?;
        let action = proposed_action("tenant_ctx", "user_ctx", "action_ctx", 1);
        repository
            .insert_proposed_action(&action, Some(ms(1_748_250_030_000)))
            .await?;
        repository
            .insert_proposed_action_evidence_ref("tenant_ctx", "action_ctx", 1, "evidence_ctx")
            .await?;
        repository
            .upsert_review_inbox_item(&inbox_item(InboxItemSpec {
                id: "inbox_ctx",
                tenant_id: "tenant_ctx",
                user_id: "user_ctx",
                proposed_action_id: "action_ctx",
                proposed_action_version: 1,
                sort_key: 600,
                sync_cursor: 601,
                status: ReviewInboxItemStatus::Open,
                ledger_status: None,
                operation_id: None,
            }))
            .await?;

        let context = recorder
            .load_review_decision_context(PostgresReviewDecisionContextRequest {
                tenant_id: "tenant_ctx",
                user_id: "user_ctx",
                proposed_action_id: "action_ctx",
                proposed_action_version: 1,
                expected_sync_cursor_value: 601,
            })
            .await?
            .expect("matching context should load");

        assert_eq!(context.item.id, "inbox_ctx");
        assert_eq!(context.item.sync_cursor_value, 601);
        assert_eq!(context.action.review_item_id, "inbox_ctx");
        assert_eq!(context.action.id, "action_ctx");
        assert_eq!(context.action.version, 1);
        assert_eq!(
            context.action.evidence_ids,
            vec!["evidence_ctx".to_string()]
        );
        assert_eq!(context.evidence.len(), 1);
        assert_eq!(
            context.evidence[0].item.summary,
            "Decision context evidence"
        );

        for stale_or_cross_scope in [
            PostgresReviewDecisionContextRequest {
                tenant_id: "tenant_ctx",
                user_id: "user_ctx",
                proposed_action_id: "action_ctx",
                proposed_action_version: 1,
                expected_sync_cursor_value: 600,
            },
            PostgresReviewDecisionContextRequest {
                tenant_id: "tenant_ctx",
                user_id: "user_ctx_other",
                proposed_action_id: "action_ctx",
                proposed_action_version: 1,
                expected_sync_cursor_value: 601,
            },
            PostgresReviewDecisionContextRequest {
                tenant_id: "tenant_ctx_other",
                user_id: "user_ctx",
                proposed_action_id: "action_ctx",
                proposed_action_version: 1,
                expected_sync_cursor_value: 601,
            },
            PostgresReviewDecisionContextRequest {
                tenant_id: "tenant_ctx",
                user_id: "user_ctx",
                proposed_action_id: "action_ctx",
                proposed_action_version: 2,
                expected_sync_cursor_value: 601,
            },
        ] {
            let context = recorder
                .load_review_decision_context(stale_or_cross_scope)
                .await?;
            assert_eq!(context, None);
        }

        Ok(())
    });
}

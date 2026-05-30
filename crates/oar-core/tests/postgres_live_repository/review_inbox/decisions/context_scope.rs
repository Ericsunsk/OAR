use super::super::*;

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

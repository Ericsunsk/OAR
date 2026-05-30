use super::*;

#[test]
fn postgres_live_audit_outbox_guarded_mark_rejects_stale_claim_after_reclaim() {
    run_live_postgres_test("audit_outbox_guarded_stale", |pool| async move {
        seed_user(&pool, "tenant_outbox_guard", "user_outbox_guard").await?;

        let repository = PostgresAuditEventRepository::new(pool.clone());
        let message_id = repository
            .enqueue_outbox(
                "tenant_outbox_guard",
                "audit-events",
                "trace_guarded",
                &json!({ "trace_id": "trace_guarded" }),
                1_000,
            )
            .await?;

        let first_claim = repository
            .claim_outbox("tenant_outbox_guard", "audit-events", 5_000, 1, 8_000)
            .await?;
        assert_eq!(first_claim.len(), 1);
        assert_eq!(first_claim[0].id, message_id);
        assert_eq!(first_claim[0].attempt_count, 1);

        let second_claim = repository
            .claim_outbox("tenant_outbox_guard", "audit-events", 9_000, 1, 12_000)
            .await?;
        assert_eq!(second_claim.len(), 1);
        assert_eq!(second_claim[0].id, message_id);
        assert_eq!(second_claim[0].attempt_count, 2);

        assert!(
            !repository
                .mark_outbox_sent_for_attempt(
                    "tenant_outbox_guard",
                    message_id,
                    first_claim[0].attempt_count,
                    8_000,
                    9_500,
                )
                .await?,
            "stale worker must not be able to mark a re-claimed message sent"
        );

        let row = sqlx::query(
            r#"
            SELECT status, attempt_count
            FROM audit_outbox
            WHERE id = $1
            "#,
        )
        .bind(message_id)
        .fetch_one(&pool)
        .await?;
        assert_eq!(row.try_get::<String, _>("status")?, "pending");
        assert_eq!(row.try_get::<i32, _>("attempt_count")?, 2);

        assert!(
            repository
                .mark_outbox_sent_for_attempt(
                    "tenant_outbox_guard",
                    message_id,
                    second_claim[0].attempt_count,
                    12_000,
                    12_500,
                )
                .await?,
            "current claimant should be able to finalize delivery"
        );

        assert!(
            !repository
                .mark_outbox_retryable_for_attempt(
                    "tenant_outbox_guard",
                    message_id,
                    second_claim[0].attempt_count,
                    12_000,
                    13_000,
                )
                .await?,
            "terminal sent messages should not be reopened as retryable"
        );

        Ok(())
    });
}

#[test]
fn postgres_live_audit_outbox_guarded_finalize_only_succeeds_once_for_same_claim() {
    run_live_postgres_test("audit_outbox_guarded_single_finalize", |pool| async move {
        seed_user(
            &pool,
            "tenant_outbox_single_finalize",
            "user_outbox_single_finalize",
        )
        .await?;

        let repository = PostgresAuditEventRepository::new(pool);
        let message_id = repository
            .enqueue_outbox(
                "tenant_outbox_single_finalize",
                "audit-events",
                "trace_single_finalize",
                &json!({ "trace_id": "trace_single_finalize" }),
                1_000,
            )
            .await?;
        let claim = repository
            .claim_outbox(
                "tenant_outbox_single_finalize",
                "audit-events",
                5_000,
                1,
                8_000,
            )
            .await?;
        assert_eq!(claim.len(), 1);
        assert_eq!(claim[0].attempt_count, 1);

        let first_mark = repository
            .mark_outbox_sent_for_attempt(
                "tenant_outbox_single_finalize",
                message_id,
                claim[0].attempt_count,
                8_000,
                8_100,
            )
            .await?;
        let duplicate_mark = repository
            .mark_outbox_sent_for_attempt(
                "tenant_outbox_single_finalize",
                message_id,
                claim[0].attempt_count,
                8_000,
                8_200,
            )
            .await?;

        assert!(first_mark);
        assert!(
            !duplicate_mark,
            "guarded finalize should be compare-and-set, not idempotent reopen"
        );
        assert!(
            !repository
                .mark_outbox_failed_for_attempt(
                    "tenant_outbox_single_finalize",
                    message_id,
                    claim[0].attempt_count,
                    8_000,
                )
                .await?,
            "terminal sent row should reject later failed mark for the same claim"
        );

        Ok(())
    });
}

#[test]
fn postgres_live_audit_outbox_retryable_then_reclaim_increments_attempt() {
    run_live_postgres_test("audit_outbox_retry_reclaim", |pool| async move {
        seed_user(&pool, "tenant_outbox_reclaim", "user_outbox_reclaim").await?;

        let repository = PostgresAuditEventRepository::new(pool.clone());
        let message_id = repository
            .enqueue_outbox(
                "tenant_outbox_reclaim",
                "audit-events",
                "trace_reclaim",
                &json!({ "trace_id": "trace_reclaim" }),
                1_000,
            )
            .await?;
        let first_claim = repository
            .claim_outbox("tenant_outbox_reclaim", "audit-events", 5_000, 1, 8_000)
            .await?;
        assert_eq!(first_claim.len(), 1);
        assert_eq!(first_claim[0].id, message_id);
        assert_eq!(first_claim[0].attempt_count, 1);

        assert!(
            repository
                .mark_outbox_retryable_for_attempt(
                    "tenant_outbox_reclaim",
                    message_id,
                    first_claim[0].attempt_count,
                    8_000,
                    12_000,
                )
                .await?
        );

        let too_early_claim = repository
            .claim_outbox("tenant_outbox_reclaim", "audit-events", 11_999, 1, 15_000)
            .await?;
        assert!(too_early_claim.is_empty());

        let second_claim = repository
            .claim_outbox("tenant_outbox_reclaim", "audit-events", 12_000, 1, 16_000)
            .await?;
        assert_eq!(second_claim.len(), 1);
        assert_eq!(second_claim[0].id, message_id);
        assert_eq!(second_claim[0].attempt_count, 2);
        assert_eq!(second_claim[0].next_attempt_at_ms, Some(16_000));

        let row = sqlx::query(
            r#"
            SELECT status, attempt_count
            FROM audit_outbox
            WHERE id = $1
            "#,
        )
        .bind(message_id)
        .fetch_one(&pool)
        .await?;
        assert_eq!(row.try_get::<String, _>("status")?, "pending");
        assert_eq!(row.try_get::<i32, _>("attempt_count")?, 2);

        Ok(())
    });
}

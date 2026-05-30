use super::*;

#[test]
fn postgres_live_audit_outbox_claims_and_marks_delivery_states() {
    run_live_postgres_test("audit_outbox_claim", |pool| async move {
        seed_user(&pool, "tenant_outbox_claim", "user_outbox_claim").await?;

        let repository = PostgresAuditEventRepository::new(pool.clone());
        let first_id = repository
            .enqueue_outbox(
                "tenant_outbox_claim",
                "audit-events",
                "trace_1",
                &json!({ "trace_id": "trace_1" }),
                1_000,
            )
            .await?;
        let second_id = repository
            .enqueue_outbox(
                "tenant_outbox_claim",
                "audit-events",
                "trace_2",
                &json!({ "trace_id": "trace_2" }),
                2_000,
            )
            .await?;
        let future_id = repository
            .enqueue_outbox(
                "tenant_outbox_claim",
                "audit-events",
                "trace_future",
                &json!({ "trace_id": "trace_future" }),
                10_000,
            )
            .await?;

        let first_claim = repository
            .claim_outbox("tenant_outbox_claim", "audit-events", 5_000, 1, 8_000)
            .await?;
        assert_eq!(first_claim.len(), 1);
        assert_eq!(first_claim[0].id, first_id);
        assert_eq!(first_claim[0].attempt_count, 1);
        assert_eq!(first_claim[0].next_attempt_at_ms, Some(8_000));

        let second_claim = repository
            .claim_outbox("tenant_outbox_claim", "audit-events", 5_000, 10, 9_000)
            .await?;
        assert_eq!(second_claim.len(), 1);
        assert_eq!(second_claim[0].id, second_id);

        assert!(
            repository
                .mark_outbox_sent("tenant_outbox_claim", first_id, 6_000)
                .await?
        );
        assert!(
            !repository
                .mark_outbox_sent("other_tenant", first_id, 6_000)
                .await?,
            "outbox delivery updates must be tenant scoped"
        );

        assert!(
            repository
                .mark_outbox_retryable("tenant_outbox_claim", second_id, 4_000)
                .await?
        );
        assert!(
            repository
                .mark_outbox_failed("tenant_outbox_claim", future_id)
                .await?
        );

        let final_claim = repository
            .claim_outbox("tenant_outbox_claim", "audit-events", 10_000, 10, 12_000)
            .await?;

        assert_eq!(final_claim.len(), 1);
        assert_eq!(final_claim[0].id, second_id);
        assert_eq!(final_claim[0].attempt_count, 2);
        assert_eq!(final_claim[0].payload, json!({ "trace_id": "trace_2" }));

        let rows = sqlx::query(
            r#"
            SELECT id, status
            FROM audit_outbox
            ORDER BY id ASC
            "#,
        )
        .fetch_all(&pool)
        .await?;
        let statuses: Vec<(i64, String)> = rows
            .iter()
            .map(|row| Ok((row.try_get("id")?, row.try_get("status")?)))
            .collect::<Result<_, sqlx::Error>>()?;

        assert_eq!(
            statuses,
            vec![
                (first_id, "sent".to_string()),
                (second_id, "pending".to_string()),
                (future_id, "failed".to_string())
            ]
        );

        Ok(())
    });
}

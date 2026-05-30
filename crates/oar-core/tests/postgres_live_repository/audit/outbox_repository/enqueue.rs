use super::*;

#[test]
fn postgres_live_audit_outbox_enqueue_sets_retry_defaults() {
    run_live_postgres_test("audit_outbox", |pool| async move {
        seed_user(&pool, "tenant_outbox", "user_outbox").await?;

        let repository = PostgresAuditEventRepository::new(pool.clone());
        let payload = json!({
            "event_id": "evt_outbox",
            "trace_id": "trace_outbox",
        });
        let id = repository
            .enqueue_outbox(
                "tenant_outbox",
                "audit-events",
                "trace_outbox",
                &payload,
                1_748_250_010_000,
            )
            .await?;

        let row = sqlx::query(
            r#"
            SELECT status, attempt_count, payload
            FROM audit_outbox
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_one(&pool)
        .await?;

        let status: String = row.try_get("status")?;
        let attempt_count: i32 = row.try_get("attempt_count")?;
        let stored_payload: serde_json::Value = row.try_get("payload")?;

        assert_eq!(status, "pending");
        assert_eq!(attempt_count, 0);
        assert_eq!(stored_payload, payload);

        Ok(())
    });
}

#[test]
fn postgres_live_audit_outbox_enqueue_rejects_unsafe_payload_without_insert() {
    run_live_postgres_test("audit_outbox_unsafe_payload", |pool| async move {
        seed_user(
            &pool,
            "tenant_outbox_unsafe_payload",
            "user_outbox_unsafe_payload",
        )
        .await?;

        let repository = PostgresAuditEventRepository::new(pool.clone());
        let result = repository
            .enqueue_outbox(
                "tenant_outbox_unsafe_payload",
                "audit-events",
                "trace_outbox_unsafe_payload",
                &json!({
                    "trace_id": "trace_outbox_unsafe_payload",
                    "authorization": "Bearer secret_value"
                }),
                1_748_250_011_000,
            )
            .await;

        assert!(matches!(
            result,
            Err(PostgresRepositoryError::UnsafeAuditOutboxPayload)
        ));

        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM audit_outbox
            WHERE tenant_id = $1
            "#,
        )
        .bind("tenant_outbox_unsafe_payload")
        .fetch_one(&pool)
        .await?;
        assert_eq!(count, 0);

        Ok(())
    });
}

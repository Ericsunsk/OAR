use super::super::harness::*;

#[test]
fn postgres_live_execution_recorder_rolls_back_when_audit_append_fails() {
    run_live_postgres_test("execution_recorder_rollback", |pool| async move {
        seed_user(&pool, "tenant_recorder_rollback", "user_recorder_rollback").await?;

        let recorder = PostgresExecutionRecorder::new(pool.clone());
        let ledger = PostgresOperationLedgerRepository::new(pool.clone());
        let audit = PostgresAuditEventRepository::new(pool.clone());
        let action = confirmed_action(
            "action_recorder_rollback",
            "tenant_recorder_rollback",
            "user_recorder_rollback",
            "idem_recorder_rollback",
        );
        let event = AuditEvent::confirmed_action(
            audit_context(
                "evt_duplicate",
                "trace_recorder_rollback",
                1,
                1_748_250_001_000,
                "user_recorder_rollback",
                "tenant_recorder_rollback",
                "progress_recorder_rollback",
            ),
            summary("confirmed by reviewer"),
        );

        audit.append(&event, None).await?;

        let result = recorder
            .record_confirmation(
                &action,
                1_748_250_000_000,
                "op_recorder_rollback",
                &event,
                &outbox_envelope(
                    "tenant_recorder_rollback",
                    "trace_recorder_rollback",
                    1_748_250_010_000,
                ),
            )
            .await;
        assert!(
            result.is_err(),
            "duplicate audit event id should fail the whole transaction"
        );

        let operation = ledger
            .get_by_idempotency_key("tenant_recorder_rollback", "idem_recorder_rollback")
            .await?;
        assert_eq!(
            operation, None,
            "ledger insert must roll back when audit append fails"
        );

        let outbox_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM audit_outbox
            WHERE tenant_id = $1
            "#,
        )
        .bind("tenant_recorder_rollback")
        .fetch_one(&pool)
        .await?;
        assert_eq!(outbox_count, 0, "outbox enqueue must roll back too");

        Ok(())
    });
}

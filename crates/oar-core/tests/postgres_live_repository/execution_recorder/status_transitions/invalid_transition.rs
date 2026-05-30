use super::*;

#[test]
fn postgres_live_execution_recorder_reports_explicit_invalid_transition() {
    run_live_postgres_test("execution_recorder_invalid_transition", |pool| async move {
        seed_user(
            &pool,
            "tenant_recorder_invalid_transition",
            "user_recorder_invalid_transition",
        )
        .await?;

        let recorder = PostgresExecutionRecorder::new(pool.clone());
        let audit = PostgresAuditEventRepository::new(pool.clone());
        let action = confirmed_action(
            "action_recorder_invalid_transition",
            "tenant_recorder_invalid_transition",
            "user_recorder_invalid_transition",
            "idem_recorder_invalid_transition",
        );

        recorder
            .record_confirmation(
                &action,
                1_748_250_000_000,
                "op_recorder_invalid_transition",
                &AuditEvent::confirmed_action(
                    audit_context(
                        "evt_recorder_invalid_transition_1",
                        "trace_recorder_invalid_transition",
                        1,
                        1_748_250_001_000,
                        "user_recorder_invalid_transition",
                        "tenant_recorder_invalid_transition",
                        "progress_recorder_invalid_transition",
                    ),
                    summary("confirmed"),
                ),
                &outbox_envelope(
                    "tenant_recorder_invalid_transition",
                    "trace_recorder_invalid_transition",
                    1_748_250_010_000,
                ),
            )
            .await?;

        let result = recorder
            .record_success(
                "tenant_recorder_invalid_transition",
                "idem_recorder_invalid_transition",
                1_748_250_003_000,
                &AuditEvent::execution_succeeded(
                    audit_context(
                        "evt_recorder_invalid_transition_2",
                        "trace_recorder_invalid_transition",
                        2,
                        1_748_250_003_000,
                        "user_recorder_invalid_transition",
                        "tenant_recorder_invalid_transition",
                        "progress_recorder_invalid_transition",
                    ),
                    Some(summary("before")),
                    Some(summary("applied")),
                    "lark_op_invalid_transition",
                ),
                &outbox_envelope(
                    "tenant_recorder_invalid_transition",
                    "trace_recorder_invalid_transition",
                    1_748_250_012_000,
                ),
            )
            .await;

        assert!(matches!(
            result,
            Err(PostgresRepositoryError::InvalidOperationStatusTransition {
                from: ActionStatus::Confirmed,
                to: ActionStatus::Succeeded,
            })
        ));

        let events = audit
            .find_by_tenant_and_trace_id(
                "tenant_recorder_invalid_transition",
                "trace_recorder_invalid_transition",
            )
            .await?;
        assert_eq!(events.len(), 1);

        let outbox_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM audit_outbox
            WHERE tenant_id = $1
            "#,
        )
        .bind("tenant_recorder_invalid_transition")
        .fetch_one(&pool)
        .await?;
        assert_eq!(outbox_count, 1);

        Ok(())
    });
}

use super::*;

#[test]
fn postgres_live_execution_recorder_records_failure_terminal_idempotently() {
    run_live_postgres_test("execution_recorder_failure", |pool| async move {
        seed_user(&pool, "tenant_recorder_failure", "user_recorder_failure").await?;

        let recorder = PostgresExecutionRecorder::new(pool.clone());
        let audit = PostgresAuditEventRepository::new(pool.clone());
        let action = confirmed_action(
            "action_recorder_failure",
            "tenant_recorder_failure",
            "user_recorder_failure",
            "idem_recorder_failure",
        );

        recorder
            .record_confirmation(
                &action,
                1_748_250_000_000,
                "op_recorder_failure",
                &AuditEvent::confirmed_action(
                    audit_context(
                        "evt_recorder_failure_1",
                        "trace_recorder_failure",
                        1,
                        1_748_250_001_000,
                        "user_recorder_failure",
                        "tenant_recorder_failure",
                        "progress_recorder_failure",
                    ),
                    summary("confirmed"),
                ),
                &outbox_envelope(
                    "tenant_recorder_failure",
                    "trace_recorder_failure",
                    1_748_250_010_000,
                ),
            )
            .await?;
        recorder
            .record_dry_run(
                "tenant_recorder_failure",
                "idem_recorder_failure",
                1_748_250_002_000,
                &AuditEvent::dry_run(
                    audit_context(
                        "evt_recorder_failure_2",
                        "trace_recorder_failure",
                        2,
                        1_748_250_002_000,
                        "user_recorder_failure",
                        "tenant_recorder_failure",
                        "progress_recorder_failure",
                    ),
                    Some(summary("before")),
                    Some(summary("projected")),
                ),
                &outbox_envelope(
                    "tenant_recorder_failure",
                    "trace_recorder_failure",
                    1_748_250_011_000,
                ),
            )
            .await?;

        let failed = recorder
            .record_failure(
                "tenant_recorder_failure",
                "idem_recorder_failure",
                "stderr leaked refresh_token=raw-secret",
                1_748_250_003_000,
                &AuditEvent::execution_failed(
                    audit_context(
                        "evt_recorder_failure_3",
                        "trace_recorder_failure",
                        3,
                        1_748_250_003_000,
                        "user_recorder_failure",
                        "tenant_recorder_failure",
                        "progress_recorder_failure",
                    ),
                    Some(summary("before")),
                    None,
                    "adapter_timeout",
                    "adapter timeout",
                ),
                &outbox_envelope(
                    "tenant_recorder_failure",
                    "trace_recorder_failure",
                    1_748_250_012_000,
                ),
            )
            .await?;
        assert_eq!(failed.operation.status, ActionStatus::Failed);
        assert_eq!(
            failed.operation.last_error.as_deref(),
            Some("adapter execution failed")
        );
        assert!(failed.outbox_id.is_some());

        let duplicate_failed = recorder
            .record_failure(
                "tenant_recorder_failure",
                "idem_recorder_failure",
                "different retry error",
                1_748_250_004_000,
                &AuditEvent::execution_failed(
                    audit_context(
                        "evt_recorder_failure_4",
                        "trace_recorder_failure",
                        4,
                        1_748_250_004_000,
                        "user_recorder_failure",
                        "tenant_recorder_failure",
                        "progress_recorder_failure",
                    ),
                    Some(summary("before")),
                    None,
                    "adapter_retry_timeout",
                    "different retry error",
                ),
                &outbox_envelope(
                    "tenant_recorder_failure",
                    "trace_recorder_failure",
                    1_748_250_013_000,
                ),
            )
            .await?;
        assert!(duplicate_failed.duplicate);
        assert_eq!(duplicate_failed.outbox_id, None);
        assert_eq!(
            duplicate_failed.operation.last_error.as_deref(),
            Some("adapter execution failed")
        );

        let events = audit
            .find_by_tenant_and_trace_id("tenant_recorder_failure", "trace_recorder_failure")
            .await?;
        assert_eq!(events.len(), 3);
        assert_eq!(events[2].event_id, "evt_recorder_failure_3");

        let outbox_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM audit_outbox
            WHERE tenant_id = $1
            "#,
        )
        .bind("tenant_recorder_failure")
        .fetch_one(&pool)
        .await?;
        assert_eq!(outbox_count, 3);

        Ok(())
    });
}

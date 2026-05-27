use super::super::harness::*;

#[test]
fn postgres_live_execution_recorder_records_dry_run_and_success_terminal_idempotently() {
    run_live_postgres_test("execution_recorder_success", |pool| async move {
        seed_user(&pool, "tenant_recorder_success", "user_recorder_success").await?;

        let recorder = PostgresExecutionRecorder::new(pool.clone());
        let ledger = PostgresOperationLedgerRepository::new(pool.clone());
        let audit = PostgresAuditEventRepository::new(pool.clone());
        let action = confirmed_action(
            "action_recorder_success",
            "tenant_recorder_success",
            "user_recorder_success",
            "idem_recorder_success",
        );

        recorder
            .record_confirmation(
                &action,
                1_748_250_000_000,
                "op_recorder_success",
                &AuditEvent::confirmed_action(
                    audit_context(
                        "evt_recorder_success_1",
                        "trace_recorder_success",
                        1,
                        1_748_250_001_000,
                        "user_recorder_success",
                        "tenant_recorder_success",
                        "progress_recorder_success",
                    ),
                    summary("confirmed"),
                ),
                &outbox_envelope(
                    "tenant_recorder_success",
                    "trace_recorder_success",
                    1_748_250_010_000,
                ),
            )
            .await?;

        let dry_run = recorder
            .record_dry_run(
                "tenant_recorder_success",
                "idem_recorder_success",
                1_748_250_002_000,
                &AuditEvent::dry_run(
                    audit_context(
                        "evt_recorder_success_2",
                        "trace_recorder_success",
                        2,
                        1_748_250_002_000,
                        "user_recorder_success",
                        "tenant_recorder_success",
                        "progress_recorder_success",
                    ),
                    Some(summary("before")),
                    Some(summary("projected")),
                ),
                &outbox_envelope(
                    "tenant_recorder_success",
                    "trace_recorder_success",
                    1_748_250_011_000,
                ),
            )
            .await?;
        assert_eq!(dry_run.operation.status, ActionStatus::Executing);
        assert!(!dry_run.duplicate);
        assert!(dry_run.outbox_id.is_some());

        let success = recorder
            .record_success(
                "tenant_recorder_success",
                "idem_recorder_success",
                1_748_250_003_000,
                &AuditEvent::execution_succeeded(
                    audit_context(
                        "evt_recorder_success_3",
                        "trace_recorder_success",
                        3,
                        1_748_250_003_000,
                        "user_recorder_success",
                        "tenant_recorder_success",
                        "progress_recorder_success",
                    ),
                    Some(summary("before")),
                    Some(summary("applied")),
                    "lark_op_success",
                ),
                &outbox_envelope(
                    "tenant_recorder_success",
                    "trace_recorder_success",
                    1_748_250_012_000,
                ),
            )
            .await?;
        assert_eq!(success.operation.status, ActionStatus::Succeeded);
        assert!(!success.duplicate);
        assert!(success.outbox_id.is_some());

        let duplicate_success = recorder
            .record_success(
                "tenant_recorder_success",
                "idem_recorder_success",
                1_748_250_004_000,
                &AuditEvent::execution_succeeded(
                    audit_context(
                        "evt_recorder_success_4",
                        "trace_recorder_success",
                        4,
                        1_748_250_004_000,
                        "user_recorder_success",
                        "tenant_recorder_success",
                        "progress_recorder_success",
                    ),
                    Some(summary("before")),
                    Some(summary("applied again")),
                    "lark_op_success_retry",
                ),
                &outbox_envelope(
                    "tenant_recorder_success",
                    "trace_recorder_success",
                    1_748_250_013_000,
                ),
            )
            .await?;
        assert_eq!(duplicate_success.operation.status, ActionStatus::Succeeded);
        assert!(duplicate_success.duplicate);
        assert_eq!(duplicate_success.outbox_id, None);

        let operation = ledger
            .get_by_idempotency_key("tenant_recorder_success", "idem_recorder_success")
            .await?
            .expect("operation should exist");
        assert_eq!(operation.status, ActionStatus::Succeeded);

        let events = audit
            .find_by_tenant_and_trace_id("tenant_recorder_success", "trace_recorder_success")
            .await?;
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].event_id, "evt_recorder_success_1");
        assert_eq!(events[1].event_id, "evt_recorder_success_2");
        assert_eq!(events[2].event_id, "evt_recorder_success_3");

        let outbox_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM audit_outbox
            WHERE tenant_id = $1
            "#,
        )
        .bind("tenant_recorder_success")
        .fetch_one(&pool)
        .await?;
        assert_eq!(outbox_count, 3);

        Ok(())
    });
}

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

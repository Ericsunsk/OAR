use super::harness::*;

#[test]
fn postgres_live_execution_uow_commits_ledger_audit_and_outbox_atomically() {
    run_live_postgres_test("execution_uow_commit", |pool| async move {
        seed_user(&pool, "tenant_uow", "user_uow").await?;

        let uow = PostgresExecutionUnitOfWork::new(pool.clone());
        let ledger = PostgresOperationLedgerRepository::new(pool.clone());
        let audit = PostgresAuditEventRepository::new(pool.clone());
        let action = confirmed_action("action_uow", "tenant_uow", "user_uow", "idem_uow");
        let event = AuditEvent::confirmed_action(
            audit_context(
                "evt_uow_1",
                "trace_uow",
                1,
                1_748_250_001_000,
                "user_uow",
                "tenant_uow",
                "progress_uow",
            ),
            summary("confirmed by reviewer"),
        );
        let outbox = outbox_envelope("tenant_uow", "trace_uow", 1_748_250_010_000);

        let report = uow
            .record_confirmation(&action, 1_748_250_000_000, "op_uow", &event, &outbox)
            .await?;

        assert_eq!(report.operation.operation_id, "op_uow");
        assert!(!report.duplicate);
        let outbox_id = report.outbox_id.expect("outbox should be enqueued");
        assert!(outbox_id > 0);

        let operation = ledger
            .get_by_idempotency_key("tenant_uow", "idem_uow")
            .await?
            .expect("operation should commit");
        assert_eq!(operation.operation_id, "op_uow");

        let events = audit.find_by_trace_id("trace_uow").await?;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_id, "evt_uow_1");

        let outbox_row = sqlx::query(
            r#"
            SELECT aggregate_id, status
            FROM audit_outbox
            WHERE id = $1
            "#,
        )
        .bind(outbox_id)
        .fetch_one(&pool)
        .await?;
        let aggregate_id: String = outbox_row.try_get("aggregate_id")?;
        let status: String = outbox_row.try_get("status")?;
        assert_eq!(aggregate_id, "trace_uow");
        assert_eq!(status, "pending");

        Ok(())
    });
}

#[test]
fn postgres_live_execution_uow_duplicate_confirmation_skips_side_effects() {
    run_live_postgres_test("execution_uow_duplicate_confirmation", |pool| async move {
        seed_user(&pool, "tenant_uow_dup", "user_uow_dup").await?;

        let uow = PostgresExecutionUnitOfWork::new(pool.clone());
        let audit = PostgresAuditEventRepository::new(pool.clone());
        let action = confirmed_action(
            "action_uow_dup",
            "tenant_uow_dup",
            "user_uow_dup",
            "idem_uow_dup",
        );
        let first_event = AuditEvent::confirmed_action(
            audit_context(
                "evt_uow_dup_1",
                "trace_uow_dup",
                1,
                1_748_250_001_000,
                "user_uow_dup",
                "tenant_uow_dup",
                "progress_uow_dup",
            ),
            summary("first confirmation"),
        );
        let second_event = AuditEvent::confirmed_action(
            audit_context(
                "evt_uow_dup_2",
                "trace_uow_dup",
                2,
                1_748_250_002_000,
                "user_uow_dup",
                "tenant_uow_dup",
                "progress_uow_dup",
            ),
            summary("duplicate confirmation"),
        );

        let first = uow
            .record_confirmation(
                &action,
                1_748_250_000_000,
                "op_uow_dup",
                &first_event,
                &outbox_envelope("tenant_uow_dup", "trace_uow_dup", 1_748_250_010_000),
            )
            .await?;
        let duplicate = uow
            .record_confirmation(
                &action,
                1_748_250_000_000,
                "op_uow_dup_retry",
                &second_event,
                &outbox_envelope("tenant_uow_dup", "trace_uow_dup", 1_748_250_011_000),
            )
            .await?;

        assert!(!first.duplicate);
        assert!(first.outbox_id.is_some());
        assert!(duplicate.duplicate);
        assert_eq!(duplicate.outbox_id, None);
        assert_eq!(duplicate.operation.operation_id, "op_uow_dup");

        let events = audit.find_by_trace_id("trace_uow_dup").await?;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_id, "evt_uow_dup_1");

        let outbox_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM audit_outbox
            WHERE tenant_id = $1
            "#,
        )
        .bind("tenant_uow_dup")
        .fetch_one(&pool)
        .await?;
        assert_eq!(outbox_count, 1);

        Ok(())
    });
}

#[test]
fn postgres_live_execution_uow_rejects_cross_tenant_event_and_outbox() {
    run_live_postgres_test("execution_uow_tenant_mismatch", |pool| async move {
        seed_user(&pool, "tenant_uow_safe", "user_uow_safe").await?;
        seed_user(&pool, "tenant_uow_other", "user_uow_other").await?;

        let uow = PostgresExecutionUnitOfWork::new(pool.clone());
        let ledger = PostgresOperationLedgerRepository::new(pool.clone());
        let action = confirmed_action(
            "action_uow_safe",
            "tenant_uow_safe",
            "user_uow_safe",
            "idem_uow_safe",
        );
        let wrong_event = AuditEvent::confirmed_action(
            audit_context(
                "evt_uow_wrong_tenant",
                "trace_uow_wrong_tenant",
                1,
                1_748_250_001_000,
                "user_uow_other",
                "tenant_uow_other",
                "progress_uow_wrong_tenant",
            ),
            summary("wrong tenant event"),
        );

        let result = uow
            .record_confirmation(
                &action,
                1_748_250_000_000,
                "op_uow_safe",
                &wrong_event,
                &outbox_envelope(
                    "tenant_uow_safe",
                    "trace_uow_wrong_tenant",
                    1_748_250_010_000,
                ),
            )
            .await;
        assert!(
            result
                .as_ref()
                .err()
                .map(|error| error.to_string().contains("tenant mismatch"))
                .unwrap_or(false),
            "tenant mismatch should be rejected before persistence"
        );

        let operation = ledger
            .get_by_idempotency_key("tenant_uow_safe", "idem_uow_safe")
            .await?;
        assert_eq!(operation, None);

        let wrong_outbox_result = uow
            .record_confirmation(
                &action,
                1_748_250_000_000,
                "op_uow_safe",
                &AuditEvent::confirmed_action(
                    audit_context(
                        "evt_uow_correct_tenant",
                        "trace_uow_wrong_outbox",
                        1,
                        1_748_250_001_000,
                        "user_uow_safe",
                        "tenant_uow_safe",
                        "progress_uow_wrong_outbox",
                    ),
                    summary("correct tenant event"),
                ),
                &outbox_envelope(
                    "tenant_uow_other",
                    "trace_uow_wrong_outbox",
                    1_748_250_010_000,
                ),
            )
            .await;
        assert!(
            wrong_outbox_result
                .as_ref()
                .err()
                .map(|error| error.to_string().contains("tenant mismatch"))
                .unwrap_or(false),
            "outbox tenant mismatch should be rejected before persistence"
        );

        let operation = ledger
            .get_by_idempotency_key("tenant_uow_safe", "idem_uow_safe")
            .await?;
        assert_eq!(operation, None);

        Ok(())
    });
}

#[test]
fn postgres_live_execution_uow_records_dry_run_and_success_terminal_idempotently() {
    run_live_postgres_test("execution_uow_success", |pool| async move {
        seed_user(&pool, "tenant_uow_success", "user_uow_success").await?;

        let uow = PostgresExecutionUnitOfWork::new(pool.clone());
        let ledger = PostgresOperationLedgerRepository::new(pool.clone());
        let audit = PostgresAuditEventRepository::new(pool.clone());
        let action = confirmed_action(
            "action_uow_success",
            "tenant_uow_success",
            "user_uow_success",
            "idem_uow_success",
        );

        uow.record_confirmation(
            &action,
            1_748_250_000_000,
            "op_uow_success",
            &AuditEvent::confirmed_action(
                audit_context(
                    "evt_uow_success_1",
                    "trace_uow_success",
                    1,
                    1_748_250_001_000,
                    "user_uow_success",
                    "tenant_uow_success",
                    "progress_uow_success",
                ),
                summary("confirmed"),
            ),
            &outbox_envelope("tenant_uow_success", "trace_uow_success", 1_748_250_010_000),
        )
        .await?;

        let dry_run = uow
            .record_dry_run(
                "tenant_uow_success",
                "idem_uow_success",
                1_748_250_002_000,
                &AuditEvent::dry_run(
                    audit_context(
                        "evt_uow_success_2",
                        "trace_uow_success",
                        2,
                        1_748_250_002_000,
                        "user_uow_success",
                        "tenant_uow_success",
                        "progress_uow_success",
                    ),
                    Some(summary("before")),
                    Some(summary("projected")),
                ),
                &outbox_envelope("tenant_uow_success", "trace_uow_success", 1_748_250_011_000),
            )
            .await?;
        assert_eq!(dry_run.operation.status, ActionStatus::Executing);
        assert!(!dry_run.duplicate);
        assert!(dry_run.outbox_id.is_some());

        let success = uow
            .record_success(
                "tenant_uow_success",
                "idem_uow_success",
                1_748_250_003_000,
                &AuditEvent::execution_succeeded(
                    audit_context(
                        "evt_uow_success_3",
                        "trace_uow_success",
                        3,
                        1_748_250_003_000,
                        "user_uow_success",
                        "tenant_uow_success",
                        "progress_uow_success",
                    ),
                    Some(summary("before")),
                    Some(summary("applied")),
                    "lark_op_success",
                ),
                &outbox_envelope("tenant_uow_success", "trace_uow_success", 1_748_250_012_000),
            )
            .await?;
        assert_eq!(success.operation.status, ActionStatus::Succeeded);
        assert!(!success.duplicate);
        assert!(success.outbox_id.is_some());

        let duplicate_success = uow
            .record_success(
                "tenant_uow_success",
                "idem_uow_success",
                1_748_250_004_000,
                &AuditEvent::execution_succeeded(
                    audit_context(
                        "evt_uow_success_4",
                        "trace_uow_success",
                        4,
                        1_748_250_004_000,
                        "user_uow_success",
                        "tenant_uow_success",
                        "progress_uow_success",
                    ),
                    Some(summary("before")),
                    Some(summary("applied again")),
                    "lark_op_success_retry",
                ),
                &outbox_envelope("tenant_uow_success", "trace_uow_success", 1_748_250_013_000),
            )
            .await?;
        assert_eq!(duplicate_success.operation.status, ActionStatus::Succeeded);
        assert!(duplicate_success.duplicate);
        assert_eq!(duplicate_success.outbox_id, None);

        let operation = ledger
            .get_by_idempotency_key("tenant_uow_success", "idem_uow_success")
            .await?
            .expect("operation should exist");
        assert_eq!(operation.status, ActionStatus::Succeeded);

        let events = audit.find_by_trace_id("trace_uow_success").await?;
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].event_id, "evt_uow_success_1");
        assert_eq!(events[1].event_id, "evt_uow_success_2");
        assert_eq!(events[2].event_id, "evt_uow_success_3");

        let outbox_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM audit_outbox
            WHERE tenant_id = $1
            "#,
        )
        .bind("tenant_uow_success")
        .fetch_one(&pool)
        .await?;
        assert_eq!(outbox_count, 3);

        Ok(())
    });
}

#[test]
fn postgres_live_execution_uow_records_failure_terminal_idempotently() {
    run_live_postgres_test("execution_uow_failure", |pool| async move {
        seed_user(&pool, "tenant_uow_failure", "user_uow_failure").await?;

        let uow = PostgresExecutionUnitOfWork::new(pool.clone());
        let audit = PostgresAuditEventRepository::new(pool.clone());
        let action = confirmed_action(
            "action_uow_failure",
            "tenant_uow_failure",
            "user_uow_failure",
            "idem_uow_failure",
        );

        uow.record_confirmation(
            &action,
            1_748_250_000_000,
            "op_uow_failure",
            &AuditEvent::confirmed_action(
                audit_context(
                    "evt_uow_failure_1",
                    "trace_uow_failure",
                    1,
                    1_748_250_001_000,
                    "user_uow_failure",
                    "tenant_uow_failure",
                    "progress_uow_failure",
                ),
                summary("confirmed"),
            ),
            &outbox_envelope("tenant_uow_failure", "trace_uow_failure", 1_748_250_010_000),
        )
        .await?;
        uow.record_dry_run(
            "tenant_uow_failure",
            "idem_uow_failure",
            1_748_250_002_000,
            &AuditEvent::dry_run(
                audit_context(
                    "evt_uow_failure_2",
                    "trace_uow_failure",
                    2,
                    1_748_250_002_000,
                    "user_uow_failure",
                    "tenant_uow_failure",
                    "progress_uow_failure",
                ),
                Some(summary("before")),
                Some(summary("projected")),
            ),
            &outbox_envelope("tenant_uow_failure", "trace_uow_failure", 1_748_250_011_000),
        )
        .await?;

        let failed = uow
            .record_failure(
                "tenant_uow_failure",
                "idem_uow_failure",
                "adapter timeout",
                1_748_250_003_000,
                &AuditEvent::execution_failed(
                    audit_context(
                        "evt_uow_failure_3",
                        "trace_uow_failure",
                        3,
                        1_748_250_003_000,
                        "user_uow_failure",
                        "tenant_uow_failure",
                        "progress_uow_failure",
                    ),
                    Some(summary("before")),
                    None,
                    "adapter_timeout",
                    "adapter timeout",
                ),
                &outbox_envelope("tenant_uow_failure", "trace_uow_failure", 1_748_250_012_000),
            )
            .await?;
        assert_eq!(failed.operation.status, ActionStatus::Failed);
        assert_eq!(
            failed.operation.last_error.as_deref(),
            Some("adapter timeout")
        );
        assert!(failed.outbox_id.is_some());

        let duplicate_failed = uow
            .record_failure(
                "tenant_uow_failure",
                "idem_uow_failure",
                "different retry error",
                1_748_250_004_000,
                &AuditEvent::execution_failed(
                    audit_context(
                        "evt_uow_failure_4",
                        "trace_uow_failure",
                        4,
                        1_748_250_004_000,
                        "user_uow_failure",
                        "tenant_uow_failure",
                        "progress_uow_failure",
                    ),
                    Some(summary("before")),
                    None,
                    "adapter_retry_timeout",
                    "different retry error",
                ),
                &outbox_envelope("tenant_uow_failure", "trace_uow_failure", 1_748_250_013_000),
            )
            .await?;
        assert!(duplicate_failed.duplicate);
        assert_eq!(duplicate_failed.outbox_id, None);
        assert_eq!(
            duplicate_failed.operation.last_error.as_deref(),
            Some("adapter timeout")
        );

        let events = audit.find_by_trace_id("trace_uow_failure").await?;
        assert_eq!(events.len(), 3);
        assert_eq!(events[2].event_id, "evt_uow_failure_3");

        let outbox_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM audit_outbox
            WHERE tenant_id = $1
            "#,
        )
        .bind("tenant_uow_failure")
        .fetch_one(&pool)
        .await?;
        assert_eq!(outbox_count, 3);

        Ok(())
    });
}

#[test]
fn postgres_live_execution_uow_reports_explicit_invalid_transition() {
    run_live_postgres_test("execution_uow_invalid_transition", |pool| async move {
        seed_user(
            &pool,
            "tenant_uow_invalid_transition",
            "user_uow_invalid_transition",
        )
        .await?;

        let uow = PostgresExecutionUnitOfWork::new(pool.clone());
        let audit = PostgresAuditEventRepository::new(pool.clone());
        let action = confirmed_action(
            "action_uow_invalid_transition",
            "tenant_uow_invalid_transition",
            "user_uow_invalid_transition",
            "idem_uow_invalid_transition",
        );

        uow.record_confirmation(
            &action,
            1_748_250_000_000,
            "op_uow_invalid_transition",
            &AuditEvent::confirmed_action(
                audit_context(
                    "evt_uow_invalid_transition_1",
                    "trace_uow_invalid_transition",
                    1,
                    1_748_250_001_000,
                    "user_uow_invalid_transition",
                    "tenant_uow_invalid_transition",
                    "progress_uow_invalid_transition",
                ),
                summary("confirmed"),
            ),
            &outbox_envelope(
                "tenant_uow_invalid_transition",
                "trace_uow_invalid_transition",
                1_748_250_010_000,
            ),
        )
        .await?;

        let result = uow
            .record_success(
                "tenant_uow_invalid_transition",
                "idem_uow_invalid_transition",
                1_748_250_003_000,
                &AuditEvent::execution_succeeded(
                    audit_context(
                        "evt_uow_invalid_transition_2",
                        "trace_uow_invalid_transition",
                        2,
                        1_748_250_003_000,
                        "user_uow_invalid_transition",
                        "tenant_uow_invalid_transition",
                        "progress_uow_invalid_transition",
                    ),
                    Some(summary("before")),
                    Some(summary("applied")),
                    "lark_op_invalid_transition",
                ),
                &outbox_envelope(
                    "tenant_uow_invalid_transition",
                    "trace_uow_invalid_transition",
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
            .find_by_trace_id("trace_uow_invalid_transition")
            .await?;
        assert_eq!(events.len(), 1);

        let outbox_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM audit_outbox
            WHERE tenant_id = $1
            "#,
        )
        .bind("tenant_uow_invalid_transition")
        .fetch_one(&pool)
        .await?;
        assert_eq!(outbox_count, 1);

        Ok(())
    });
}

#[test]
fn postgres_live_execution_uow_rolls_back_when_audit_append_fails() {
    run_live_postgres_test("execution_uow_rollback", |pool| async move {
        seed_user(&pool, "tenant_uow_rollback", "user_uow_rollback").await?;

        let uow = PostgresExecutionUnitOfWork::new(pool.clone());
        let ledger = PostgresOperationLedgerRepository::new(pool.clone());
        let audit = PostgresAuditEventRepository::new(pool.clone());
        let action = confirmed_action(
            "action_uow_rollback",
            "tenant_uow_rollback",
            "user_uow_rollback",
            "idem_uow_rollback",
        );
        let event = AuditEvent::confirmed_action(
            audit_context(
                "evt_duplicate",
                "trace_uow_rollback",
                1,
                1_748_250_001_000,
                "user_uow_rollback",
                "tenant_uow_rollback",
                "progress_uow_rollback",
            ),
            summary("confirmed by reviewer"),
        );

        audit.append(&event, None).await?;

        let result = uow
            .record_confirmation(
                &action,
                1_748_250_000_000,
                "op_uow_rollback",
                &event,
                &outbox_envelope(
                    "tenant_uow_rollback",
                    "trace_uow_rollback",
                    1_748_250_010_000,
                ),
            )
            .await;
        assert!(
            result.is_err(),
            "duplicate audit event id should fail the whole transaction"
        );

        let operation = ledger
            .get_by_idempotency_key("tenant_uow_rollback", "idem_uow_rollback")
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
        .bind("tenant_uow_rollback")
        .fetch_one(&pool)
        .await?;
        assert_eq!(outbox_count, 0, "outbox enqueue must roll back too");

        Ok(())
    });
}

use super::super::harness::*;

#[test]
fn postgres_live_execution_recorder_commits_ledger_audit_and_outbox_atomically() {
    run_live_postgres_test("execution_recorder_commit", |pool| async move {
        seed_user(&pool, "tenant_recorder", "user_recorder").await?;

        let recorder = PostgresExecutionRecorder::new(pool.clone());
        let ledger = PostgresOperationLedgerRepository::new(pool.clone());
        let audit = PostgresAuditEventRepository::new(pool.clone());
        let action = confirmed_action(
            "action_recorder",
            "tenant_recorder",
            "user_recorder",
            "idem_recorder",
        );
        let event = AuditEvent::confirmed_action(
            audit_context(
                "evt_recorder_1",
                "trace_recorder",
                1,
                1_748_250_001_000,
                "user_recorder",
                "tenant_recorder",
                "progress_recorder",
            ),
            summary("confirmed by reviewer"),
        );
        let outbox = outbox_envelope("tenant_recorder", "trace_recorder", 1_748_250_010_000);

        let report = recorder
            .record_confirmation(&action, 1_748_250_000_000, "op_recorder", &event, &outbox)
            .await?;

        assert_eq!(report.operation.operation_id, "op_recorder");
        assert!(!report.duplicate);
        let outbox_id = report.outbox_id.expect("outbox should be enqueued");
        assert!(outbox_id > 0);

        let operation = ledger
            .get_by_idempotency_key("tenant_recorder", "idem_recorder")
            .await?
            .expect("operation should commit");
        assert_eq!(operation.operation_id, "op_recorder");

        let events = audit
            .find_by_tenant_and_trace_id("tenant_recorder", "trace_recorder")
            .await?;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_id, "evt_recorder_1");

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
        assert_eq!(aggregate_id, "trace_recorder");
        assert_eq!(status, "pending");

        Ok(())
    });
}

#[test]
fn postgres_live_execution_recorder_duplicate_confirmation_skips_side_effects() {
    run_live_postgres_test(
        "execution_recorder_duplicate_confirmation",
        |pool| async move {
            seed_user(&pool, "tenant_recorder_dup", "user_recorder_dup").await?;

            let recorder = PostgresExecutionRecorder::new(pool.clone());
            let audit = PostgresAuditEventRepository::new(pool.clone());
            let action = confirmed_action(
                "action_recorder_dup",
                "tenant_recorder_dup",
                "user_recorder_dup",
                "idem_recorder_dup",
            );
            let first_event = AuditEvent::confirmed_action(
                audit_context(
                    "evt_recorder_dup_1",
                    "trace_recorder_dup",
                    1,
                    1_748_250_001_000,
                    "user_recorder_dup",
                    "tenant_recorder_dup",
                    "progress_recorder_dup",
                ),
                summary("first confirmation"),
            );
            let second_event = AuditEvent::confirmed_action(
                audit_context(
                    "evt_recorder_dup_2",
                    "trace_recorder_dup",
                    2,
                    1_748_250_002_000,
                    "user_recorder_dup",
                    "tenant_recorder_dup",
                    "progress_recorder_dup",
                ),
                summary("duplicate confirmation"),
            );

            let first = recorder
                .record_confirmation(
                    &action,
                    1_748_250_000_000,
                    "op_recorder_dup",
                    &first_event,
                    &outbox_envelope(
                        "tenant_recorder_dup",
                        "trace_recorder_dup",
                        1_748_250_010_000,
                    ),
                )
                .await?;
            let duplicate = recorder
                .record_confirmation(
                    &action,
                    1_748_250_000_000,
                    "op_recorder_dup_retry",
                    &second_event,
                    &outbox_envelope(
                        "tenant_recorder_dup",
                        "trace_recorder_dup",
                        1_748_250_011_000,
                    ),
                )
                .await?;

            assert!(!first.duplicate);
            assert!(first.outbox_id.is_some());
            assert!(duplicate.duplicate);
            assert_eq!(duplicate.outbox_id, None);
            assert_eq!(duplicate.operation.operation_id, "op_recorder_dup");

            let events = audit
                .find_by_tenant_and_trace_id("tenant_recorder_dup", "trace_recorder_dup")
                .await?;
            assert_eq!(events.len(), 1);
            assert_eq!(events[0].event_id, "evt_recorder_dup_1");

            let outbox_count: i64 = sqlx::query_scalar(
                r#"
            SELECT COUNT(*)
            FROM audit_outbox
            WHERE tenant_id = $1
            "#,
            )
            .bind("tenant_recorder_dup")
            .fetch_one(&pool)
            .await?;
            assert_eq!(outbox_count, 1);

            Ok(())
        },
    );
}

#[test]
fn postgres_live_execution_recorder_rejects_cross_tenant_event_and_outbox() {
    run_live_postgres_test("execution_recorder_tenant_mismatch", |pool| async move {
        seed_user(&pool, "tenant_recorder_safe", "user_recorder_safe").await?;
        seed_user(&pool, "tenant_recorder_other", "user_recorder_other").await?;

        let recorder = PostgresExecutionRecorder::new(pool.clone());
        let ledger = PostgresOperationLedgerRepository::new(pool.clone());
        let action = confirmed_action(
            "action_recorder_safe",
            "tenant_recorder_safe",
            "user_recorder_safe",
            "idem_recorder_safe",
        );
        let wrong_event = AuditEvent::confirmed_action(
            audit_context(
                "evt_recorder_wrong_tenant",
                "trace_recorder_wrong_tenant",
                1,
                1_748_250_001_000,
                "user_recorder_other",
                "tenant_recorder_other",
                "progress_recorder_wrong_tenant",
            ),
            summary("wrong tenant event"),
        );

        let result = recorder
            .record_confirmation(
                &action,
                1_748_250_000_000,
                "op_recorder_safe",
                &wrong_event,
                &outbox_envelope(
                    "tenant_recorder_safe",
                    "trace_recorder_wrong_tenant",
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
            .get_by_idempotency_key("tenant_recorder_safe", "idem_recorder_safe")
            .await?;
        assert_eq!(operation, None);

        let wrong_outbox_result = recorder
            .record_confirmation(
                &action,
                1_748_250_000_000,
                "op_recorder_safe",
                &AuditEvent::confirmed_action(
                    audit_context(
                        "evt_recorder_correct_tenant",
                        "trace_recorder_wrong_outbox",
                        1,
                        1_748_250_001_000,
                        "user_recorder_safe",
                        "tenant_recorder_safe",
                        "progress_recorder_wrong_outbox",
                    ),
                    summary("correct tenant event"),
                ),
                &outbox_envelope(
                    "tenant_recorder_other",
                    "trace_recorder_wrong_outbox",
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
            .get_by_idempotency_key("tenant_recorder_safe", "idem_recorder_safe")
            .await?;
        assert_eq!(operation, None);

        Ok(())
    });
}

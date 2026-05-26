use super::harness::*;

#[test]
fn postgres_live_action_executor_records_success_audit_and_outbox() {
    run_live_postgres_test("action_executor_success", |pool| async move {
        seed_user(&pool, "tenant_executor_success", "user_executor_success").await?;

        let adapter = LiveMockAdapter::succeeding();
        let mut executor = postgres_action_executor(pool.clone(), adapter.clone());
        let audit = PostgresAuditEventRepository::new(pool.clone());
        let action = confirmed_action(
            "action_executor_success",
            "tenant_executor_success",
            "user_executor_success",
            "idem_executor_success",
        );

        let report = executor.execute_confirmed_action(&action).await.unwrap();

        assert!(!report.duplicate);
        assert_eq!(report.operation.status, ActionStatus::Succeeded);
        assert_eq!(adapter.dry_run_calls(), 1);
        assert_eq!(adapter.execute_calls(), 1);
        assert_eq!(report.events.len(), 3);
        assert_eq!(
            report.events[0].event_type,
            AuditEventType::ConfirmedActionRecorded
        );
        assert_eq!(report.events[1].event_type, AuditEventType::DryRunExecuted);
        assert_eq!(
            report.events[2].event_type,
            AuditEventType::ExecutionSucceeded
        );
        assert_eq!(
            report.events[2]
                .execution
                .as_ref()
                .and_then(|execution| execution.adapter_operation_id.as_deref()),
            Some("lark-op-live")
        );

        let persisted = audit
            .find_by_trace_id("trace-idem_executor_success")
            .await?;
        assert_eq!(persisted, report.events);
        assert_eq!(
            audit_outbox_count(&pool, "tenant_executor_success").await?,
            3
        );

        let linked_event_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM audit_events
            WHERE trace_id = $1 AND operation_id = $2
            "#,
        )
        .bind("trace-idem_executor_success")
        .bind(&report.operation.operation_id)
        .fetch_one(&pool)
        .await?;
        assert_eq!(linked_event_count, 3);

        Ok(())
    });
}

#[test]
fn postgres_live_action_executor_duplicate_retry_skips_adapter_and_side_effects() {
    run_live_postgres_test("action_executor_duplicate", |pool| async move {
        seed_user(&pool, "tenant_executor_dup", "user_executor_dup").await?;

        let adapter = LiveMockAdapter::succeeding();
        let action = confirmed_action(
            "action_executor_dup",
            "tenant_executor_dup",
            "user_executor_dup",
            "idem_executor_dup",
        );
        let mut first_executor = postgres_action_executor(pool.clone(), adapter.clone());
        let mut retry_executor = postgres_action_executor(pool.clone(), adapter.clone());

        let first = first_executor
            .execute_confirmed_action(&action)
            .await
            .unwrap();
        let retry = retry_executor
            .execute_confirmed_action(&action)
            .await
            .unwrap();

        assert!(!first.duplicate);
        assert!(retry.duplicate);
        assert!(retry.events.is_empty());
        assert_eq!(first.operation.operation_id, retry.operation.operation_id);
        assert_eq!(adapter.dry_run_calls(), 1);
        assert_eq!(adapter.execute_calls(), 1);

        let audit = PostgresAuditEventRepository::new(pool.clone());
        let events = audit.find_by_trace_id("trace-idem_executor_dup").await?;
        assert_eq!(events.len(), 3);
        assert_eq!(audit_outbox_count(&pool, "tenant_executor_dup").await?, 3);

        Ok(())
    });
}

#[test]
fn postgres_live_action_executor_resumes_after_confirmation_only_crash() {
    run_live_postgres_test("action_executor_confirmation_resume", |pool| async move {
        seed_user(&pool, "tenant_executor_resume", "user_executor_resume").await?;

        let action = confirmed_action(
            "action_executor_resume",
            "tenant_executor_resume",
            "user_executor_resume",
            "idem_executor_resume",
        );
        let uow = PostgresExecutionUnitOfWork::new(pool.clone());
        uow.record_confirmation(
            &action,
            1_748_260_000_000,
            "op-idem_executor_resume",
            &AuditEvent::confirmed_action(
                audit_context(
                    "trace-idem_executor_resume-evt-1",
                    "trace-idem_executor_resume",
                    1,
                    1_748_260_001_000,
                    "user_executor_resume",
                    "tenant_executor_resume",
                    "action_executor_resume",
                ),
                summary("confirmed before crash"),
            ),
            &outbox_envelope(
                "tenant_executor_resume",
                "trace-idem_executor_resume",
                1_748_260_002_000,
            ),
        )
        .await?;

        let adapter = LiveMockAdapter::succeeding();
        let mut executor = postgres_action_executor(pool.clone(), adapter.clone());

        let report = executor.execute_confirmed_action(&action).await.unwrap();

        assert!(!report.duplicate);
        assert_eq!(report.operation.status, ActionStatus::Succeeded);
        assert_eq!(adapter.dry_run_calls(), 1);
        assert_eq!(adapter.execute_calls(), 1);
        assert_eq!(report.events.len(), 2);
        assert_eq!(report.events[0].event_type, AuditEventType::DryRunExecuted);
        assert_eq!(
            report.events[1].event_type,
            AuditEventType::ExecutionSucceeded
        );

        let audit = PostgresAuditEventRepository::new(pool.clone());
        let persisted = audit.find_by_trace_id("trace-idem_executor_resume").await?;
        assert_eq!(persisted.len(), 3);
        assert_eq!(
            persisted
                .iter()
                .map(|event| event.event_type.clone())
                .collect::<Vec<_>>(),
            vec![
                AuditEventType::ConfirmedActionRecorded,
                AuditEventType::DryRunExecuted,
                AuditEventType::ExecutionSucceeded
            ]
        );
        assert_eq!(
            audit_outbox_count(&pool, "tenant_executor_resume").await?,
            3
        );

        Ok(())
    });
}

#[test]
fn postgres_live_action_executor_resumes_existing_executing_operation() {
    run_live_postgres_test("action_executor_executing_resume", |pool| async move {
        seed_user(
            &pool,
            "tenant_executor_executing_resume",
            "user_executor_executing_resume",
        )
        .await?;

        let action = confirmed_action(
            "action_executor_executing_resume",
            "tenant_executor_executing_resume",
            "user_executor_executing_resume",
            "idem_executor_executing_resume",
        );
        let uow = PostgresExecutionUnitOfWork::new(pool.clone());
        uow.record_confirmation(
            &action,
            1_748_260_000_000,
            "op-idem_executor_executing_resume",
            &AuditEvent::confirmed_action(
                audit_context(
                    "trace-idem_executor_executing_resume-evt-1",
                    "trace-idem_executor_executing_resume",
                    1,
                    1_748_260_001_000,
                    "user_executor_executing_resume",
                    "tenant_executor_executing_resume",
                    "action_executor_executing_resume",
                ),
                summary("confirmed before crash"),
            ),
            &outbox_envelope(
                "tenant_executor_executing_resume",
                "trace-idem_executor_executing_resume",
                1_748_260_002_000,
            ),
        )
        .await?;
        uow.record_dry_run(
            "tenant_executor_executing_resume",
            "idem_executor_executing_resume",
            1_748_260_003_000,
            &AuditEvent::dry_run(
                audit_context(
                    "trace-idem_executor_executing_resume-evt-2",
                    "trace-idem_executor_executing_resume",
                    2,
                    1_748_260_003_000,
                    "user_executor_executing_resume",
                    "tenant_executor_executing_resume",
                    "action_executor_executing_resume",
                ),
                Some(summary("before crash")),
                Some(summary("projected before crash")),
            ),
            &outbox_envelope(
                "tenant_executor_executing_resume",
                "trace-idem_executor_executing_resume",
                1_748_260_004_000,
            ),
        )
        .await?;

        let adapter = LiveMockAdapter::succeeding();
        let mut executor = postgres_action_executor(pool.clone(), adapter.clone());

        let report = executor.execute_confirmed_action(&action).await.unwrap();

        assert!(!report.duplicate);
        assert_eq!(report.operation.status, ActionStatus::Succeeded);
        assert_eq!(
            adapter.dry_run_calls(),
            1,
            "executor re-runs dry-run to rebuild adapter context"
        );
        assert_eq!(adapter.execute_calls(), 1);
        assert_eq!(report.events.len(), 1);
        assert_eq!(
            report.events[0].event_type,
            AuditEventType::ExecutionSucceeded
        );

        let audit = PostgresAuditEventRepository::new(pool.clone());
        let persisted = audit
            .find_by_trace_id("trace-idem_executor_executing_resume")
            .await?;
        assert_eq!(persisted.len(), 3);
        assert_eq!(
            persisted
                .iter()
                .map(|event| event.event_type.clone())
                .collect::<Vec<_>>(),
            vec![
                AuditEventType::ConfirmedActionRecorded,
                AuditEventType::DryRunExecuted,
                AuditEventType::ExecutionSucceeded
            ]
        );
        assert_eq!(
            audit_outbox_count(&pool, "tenant_executor_executing_resume").await?,
            3
        );

        Ok(())
    });
}

#[test]
fn postgres_live_action_executor_records_adapter_failure_as_terminal_state() {
    run_live_postgres_test("action_executor_failure", |pool| async move {
        seed_user(&pool, "tenant_executor_failure", "user_executor_failure").await?;

        let adapter = LiveMockAdapter::failing("adapter_timeout", "network timeout");
        let mut executor = postgres_action_executor(pool.clone(), adapter.clone());
        let audit = PostgresAuditEventRepository::new(pool.clone());
        let action = confirmed_action(
            "action_executor_failure",
            "tenant_executor_failure",
            "user_executor_failure",
            "idem_executor_failure",
        );

        let report = executor.execute_confirmed_action(&action).await.unwrap();

        assert!(!report.duplicate);
        assert_eq!(report.operation.status, ActionStatus::Failed);
        assert_eq!(
            report.operation.last_error.as_deref(),
            Some("network timeout")
        );
        assert_eq!(adapter.dry_run_calls(), 1);
        assert_eq!(adapter.execute_calls(), 1);
        assert_eq!(report.events.len(), 3);
        assert_eq!(report.events[2].event_type, AuditEventType::ExecutionFailed);
        assert_eq!(
            report.events[2]
                .execution
                .as_ref()
                .and_then(|execution| execution.error_code.as_deref()),
            Some("adapter_timeout")
        );

        let persisted = audit
            .find_by_trace_id("trace-idem_executor_failure")
            .await?;
        assert_eq!(persisted, report.events);
        assert_eq!(
            audit_outbox_count(&pool, "tenant_executor_failure").await?,
            3
        );

        Ok(())
    });
}

#[test]
fn postgres_live_action_executor_policy_denial_records_safe_audit_without_adapter_call() {
    run_live_postgres_test("action_executor_policy_denied", |pool| async move {
        seed_user(&pool, "tenant_executor_policy", "user_executor_policy").await?;

        let adapter = LiveMockAdapter::succeeding();
        let mut executor = postgres_action_executor(pool.clone(), adapter.clone());
        let action = confirmed_action(
            "action_executor_policy",
            "tenant_executor_policy",
            "user_executor_policy",
            "idem_executor_policy",
        );
        let policy = progress_update_policy();
        let grant = token_grant(
            "tenant_executor_policy",
            &["offline_access"],
            TokenGrantState::Valid,
        );
        let binding = actor_binding("user_executor_policy");

        let result = executor
            .execute_confirmed_action_with_policy(
                &action,
                "okr.progress.update",
                "okr.progress.write",
                &binding,
                &grant,
                &policy,
            )
            .await;

        assert_eq!(adapter.dry_run_calls(), 0);
        assert_eq!(adapter.execute_calls(), 0);

        let report = match result {
            Err(ExecutionError::PolicyDenied(report)) => report,
            other => panic!("expected policy denial, got {other:?}"),
        };
        assert_eq!(
            report.denial,
            ExecutionDenied::MissingScope {
                required_scope: "okr.progress.write".to_string()
            }
        );
        assert_eq!(report.events.len(), 1);
        assert_eq!(report.events[0].event_type, AuditEventType::ExecutionDenied);
        assert_eq!(
            report.events[0]
                .execution
                .as_ref()
                .and_then(|execution| execution.error_code.as_deref()),
            Some("policy_denied")
        );
        let message = report.events[0]
            .execution
            .as_ref()
            .and_then(|execution| execution.message.as_deref())
            .unwrap_or_default();
        assert!(message.contains("policy"));
        assert!(message.contains("okr.progress.write"));
        assert!(!message.contains("access-token"));
        assert!(!message.contains("refresh-token"));

        let ledger = PostgresOperationLedgerRepository::new(pool.clone());
        assert_eq!(
            ledger
                .get_by_idempotency_key("tenant_executor_policy", "idem_executor_policy")
                .await?,
            None
        );

        let audit = PostgresAuditEventRepository::new(pool.clone());
        let persisted = audit.find_by_trace_id("trace-idem_executor_policy").await?;
        assert_eq!(persisted, report.events);
        assert_eq!(
            audit_outbox_count(&pool, "tenant_executor_policy").await?,
            0
        );

        Ok(())
    });
}

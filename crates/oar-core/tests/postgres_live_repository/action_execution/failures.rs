use super::*;

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
        let request = confirmed_execution_request(action);

        let report = executor.execute_confirmed_request(&request).await.unwrap();

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
            .find_by_tenant_and_trace_id(
                "tenant_executor_failure",
                "trace-tenant_executor_failure-idem_executor_failure",
            )
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
        let request = confirmed_execution_request(action);
        let policy = okr_progress_write_policy();
        let grant = token_grant(
            "tenant_executor_policy",
            &["offline_access"],
            TokenGrantState::Valid,
        );
        let binding = actor_binding("user_executor_policy");

        let result = executor
            .execute_confirmed_request_with_policy(
                &request,
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
        let persisted = audit
            .find_by_tenant_and_trace_id(
                "tenant_executor_policy",
                "trace-tenant_executor_policy-idem_executor_policy",
            )
            .await?;
        assert_eq!(persisted, report.events);
        assert_eq!(
            audit_outbox_count(&pool, "tenant_executor_policy").await?,
            0
        );

        Ok(())
    });
}

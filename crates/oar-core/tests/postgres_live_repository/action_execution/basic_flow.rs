use super::*;

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
        let request = confirmed_execution_request(action);

        let report = executor.execute_confirmed_request(&request).await.unwrap();

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
            .find_by_tenant_and_trace_id(
                "tenant_executor_success",
                "trace-tenant_executor_success-idem_executor_success",
            )
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
            WHERE tenant_id = $1
              AND trace_id = $2
              AND operation_id = $3
            "#,
        )
        .bind("tenant_executor_success")
        .bind("trace-tenant_executor_success-idem_executor_success")
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
        let request = confirmed_execution_request(action);
        let mut first_executor = postgres_action_executor(pool.clone(), adapter.clone());
        let mut retry_executor = postgres_action_executor(pool.clone(), adapter.clone());

        let first = first_executor
            .execute_confirmed_request(&request)
            .await
            .unwrap();
        let retry = retry_executor
            .execute_confirmed_request(&request)
            .await
            .unwrap();

        assert!(!first.duplicate);
        assert!(retry.duplicate);
        assert!(retry.events.is_empty());
        assert_eq!(first.operation.operation_id, retry.operation.operation_id);
        assert_eq!(adapter.dry_run_calls(), 1);
        assert_eq!(adapter.execute_calls(), 1);

        let audit = PostgresAuditEventRepository::new(pool.clone());
        let events = audit
            .find_by_tenant_and_trace_id(
                "tenant_executor_dup",
                "trace-tenant_executor_dup-idem_executor_dup",
            )
            .await?;
        assert_eq!(events.len(), 3);
        assert_eq!(audit_outbox_count(&pool, "tenant_executor_dup").await?, 3);

        Ok(())
    });
}

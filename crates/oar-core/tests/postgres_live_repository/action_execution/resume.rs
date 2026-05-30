use super::resume_support::*;
use super::*;

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
        seed_resume_confirmation(&pool, &action, "confirmed before crash").await?;

        let adapter = LiveMockAdapter::succeeding();
        let mut executor = postgres_action_executor(pool.clone(), adapter.clone());
        let request = confirmed_execution_request(action.clone());

        let report = executor.execute_confirmed_request(&request).await.unwrap();

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

        assert_persisted_audit_event_types(
            &pool,
            "tenant_executor_resume",
            &resume_trace_id(&action),
            &[
                AuditEventType::ConfirmedActionRecorded,
                AuditEventType::DryRunExecuted,
                AuditEventType::ExecutionSucceeded,
            ],
        )
        .await?;
        assert_eq!(
            audit_outbox_count(&pool, "tenant_executor_resume").await?,
            3
        );

        Ok(())
    });
}

#[test]
fn postgres_live_action_executor_existing_executing_operation_is_inflight_duplicate() {
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
        seed_resume_confirmation(&pool, &action, "confirmed before crash").await?;
        seed_resume_dry_run(&pool, &action, "before crash", "projected before crash").await?;

        let adapter = LiveMockAdapter::succeeding();
        let mut executor = postgres_action_executor(pool.clone(), adapter.clone());
        let request = confirmed_execution_request(action.clone());

        let report = executor.execute_confirmed_request(&request).await.unwrap();

        assert!(report.duplicate);
        assert_eq!(report.operation.status, ActionStatus::Executing);
        assert_eq!(
            adapter.dry_run_calls(),
            0,
            "executor should not touch adapter without execution ownership"
        );
        assert_eq!(adapter.execute_calls(), 0);
        assert!(report.events.is_empty());

        assert_persisted_audit_event_types(
            &pool,
            "tenant_executor_executing_resume",
            &resume_trace_id(&action),
            &[
                AuditEventType::ConfirmedActionRecorded,
                AuditEventType::DryRunExecuted,
            ],
        )
        .await?;
        assert_eq!(
            audit_outbox_count(&pool, "tenant_executor_executing_resume").await?,
            2
        );

        Ok(())
    });
}

#[test]
fn postgres_live_action_executor_dry_run_race_does_not_execute_without_ownership() {
    run_live_postgres_test("action_executor_dry_run_race", |pool| async move {
        seed_user(&pool, "tenant_executor_race", "user_executor_race").await?;

        let adapter = DryRunRaceAdapter::new(pool.clone());
        let mut tick = 1_748_260_000_000_u64;
        let mut executor = PostgresActionExecutor::new(
            adapter.clone(),
            move || {
                tick += 1_000;
                tick
            },
            PostgresExecutionRecorder::new(pool.clone()),
            PostgresAuditEventRepository::new(pool.clone()),
        );
        let action = confirmed_action(
            "action_executor_race",
            "tenant_executor_race",
            "user_executor_race",
            "idem_executor_race",
        );
        let request = confirmed_execution_request(action.clone());

        let report = executor.execute_confirmed_request(&request).await.unwrap();

        assert!(report.duplicate);
        assert_eq!(report.operation.status, ActionStatus::Executing);
        assert_eq!(adapter.dry_run_calls(), 1);
        assert_eq!(adapter.execute_calls(), 0);
        assert_eq!(report.events.len(), 1);
        assert_eq!(
            report.events[0].event_type,
            AuditEventType::ConfirmedActionRecorded
        );

        assert_persisted_audit_event_types(
            &pool,
            "tenant_executor_race",
            &resume_trace_id(&action),
            &[
                AuditEventType::ConfirmedActionRecorded,
                AuditEventType::DryRunExecuted,
            ],
        )
        .await?;
        assert_eq!(audit_outbox_count(&pool, "tenant_executor_race").await?, 2);

        Ok(())
    });
}

use super::*;

#[derive(Clone)]
struct DryRunRaceAdapter {
    pool: PgPool,
    dry_run_calls: Arc<Mutex<usize>>,
    execute_calls: Arc<Mutex<usize>>,
}

impl DryRunRaceAdapter {
    fn new(pool: PgPool) -> Self {
        Self {
            pool,
            dry_run_calls: Arc::new(Mutex::new(0)),
            execute_calls: Arc::new(Mutex::new(0)),
        }
    }

    fn dry_run_calls(&self) -> usize {
        *self.dry_run_calls.lock().expect("dry-run mutex")
    }

    fn execute_calls(&self) -> usize {
        *self.execute_calls.lock().expect("execute mutex")
    }
}

impl ActionAdapter for DryRunRaceAdapter {
    fn dry_run(
        &mut self,
        request: &ConfirmedExecutionRequest,
    ) -> Result<AdapterDryRun, AdapterError> {
        *self.dry_run_calls.lock().expect("dry-run mutex") += 1;

        let action = request.action();
        let pool = self.pool.clone();
        let tenant_id = action.tenant_id.clone();
        let idempotency_key = action.idempotency_key.clone();
        let actor_user_id = action.actor_user_id.clone();
        let action_id = action.action_id.clone();
        std::thread::spawn(move || {
            runtime().block_on(async move {
                let recorder = PostgresExecutionRecorder::new(pool);
                recorder
                    .record_dry_run(
                        &tenant_id,
                        &idempotency_key,
                        1_748_260_003_000,
                        &AuditEvent::dry_run(
                            audit_context(
                                "evt_dry_run_race_2",
                                &format!("trace-{tenant_id}-{idempotency_key}"),
                                2,
                                1_748_260_003_000,
                                &actor_user_id,
                                &tenant_id,
                                &action_id,
                            ),
                            Some(summary("race before")),
                            Some(summary("race projected")),
                        ),
                        &outbox_envelope(
                            &tenant_id,
                            &format!("trace-{tenant_id}-{idempotency_key}"),
                            1_748_260_004_000,
                        ),
                    )
                    .await
                    .expect("race dry-run should mark executing");
            });
        })
        .join()
        .expect("race thread should complete");

        Ok(AdapterDryRun {
            before: Some(summary("late before")),
            after: Some(summary("late projected")),
        })
    }

    fn execute(
        &mut self,
        _request: &ConfirmedExecutionRequest,
    ) -> Result<AdapterExecution, AdapterError> {
        *self.execute_calls.lock().expect("execute mutex") += 1;
        Ok(AdapterExecution {
            adapter_operation_id: "lark-op-race".to_string(),
            before: Some(summary("before")),
            after: Some(summary("applied")),
        })
    }
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
        let recorder = PostgresExecutionRecorder::new(pool.clone());
        recorder
            .record_confirmation(
                &action,
                1_748_260_000_000,
                "op-idem_executor_resume",
                &AuditEvent::confirmed_action(
                    audit_context(
                        "trace-tenant_executor_resume-idem_executor_resume-evt-1",
                        "trace-tenant_executor_resume-idem_executor_resume",
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
                    "trace-tenant_executor_resume-idem_executor_resume",
                    1_748_260_002_000,
                ),
            )
            .await?;

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

        let audit = PostgresAuditEventRepository::new(pool.clone());
        let persisted = audit
            .find_by_tenant_and_trace_id(
                "tenant_executor_resume",
                "trace-tenant_executor_resume-idem_executor_resume",
            )
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
        let recorder = PostgresExecutionRecorder::new(pool.clone());
        recorder.record_confirmation(
            &action,
            1_748_260_000_000,
            "op-idem_executor_executing_resume",
            &AuditEvent::confirmed_action(
                audit_context(
                    "trace-tenant_executor_executing_resume-idem_executor_executing_resume-evt-1",
                    "trace-tenant_executor_executing_resume-idem_executor_executing_resume",
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
                "trace-tenant_executor_executing_resume-idem_executor_executing_resume",
                1_748_260_002_000,
            ),
        )
        .await?;
        recorder.record_dry_run(
            "tenant_executor_executing_resume",
            "idem_executor_executing_resume",
            1_748_260_003_000,
            &AuditEvent::dry_run(
                audit_context(
                    "trace-tenant_executor_executing_resume-idem_executor_executing_resume-evt-2",
                    "trace-tenant_executor_executing_resume-idem_executor_executing_resume",
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
                "trace-tenant_executor_executing_resume-idem_executor_executing_resume",
                1_748_260_004_000,
            ),
        )
        .await?;

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

        let audit = PostgresAuditEventRepository::new(pool.clone());
        let persisted = audit
            .find_by_tenant_and_trace_id(
                "tenant_executor_executing_resume",
                "trace-tenant_executor_executing_resume-idem_executor_executing_resume",
            )
            .await?;
        assert_eq!(persisted.len(), 2);
        assert_eq!(
            persisted
                .iter()
                .map(|event| event.event_type.clone())
                .collect::<Vec<_>>(),
            vec![
                AuditEventType::ConfirmedActionRecorded,
                AuditEventType::DryRunExecuted
            ]
        );
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
        let audit = PostgresAuditEventRepository::new(pool.clone());
        let action = confirmed_action(
            "action_executor_race",
            "tenant_executor_race",
            "user_executor_race",
            "idem_executor_race",
        );
        let request = confirmed_execution_request(action);

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

        let persisted = audit
            .find_by_tenant_and_trace_id(
                "tenant_executor_race",
                "trace-tenant_executor_race-idem_executor_race",
            )
            .await?;
        assert_eq!(
            persisted
                .iter()
                .map(|event| event.event_type.clone())
                .collect::<Vec<_>>(),
            vec![
                AuditEventType::ConfirmedActionRecorded,
                AuditEventType::DryRunExecuted
            ]
        );
        assert_eq!(audit_outbox_count(&pool, "tenant_executor_race").await?, 2);

        Ok(())
    });
}

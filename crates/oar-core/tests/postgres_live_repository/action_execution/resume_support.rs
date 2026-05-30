use super::*;

#[derive(Clone)]
pub(crate) struct DryRunRaceAdapter {
    pool: PgPool,
    dry_run_calls: Arc<Mutex<usize>>,
    execute_calls: Arc<Mutex<usize>>,
}

impl DryRunRaceAdapter {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            dry_run_calls: Arc::new(Mutex::new(0)),
            execute_calls: Arc::new(Mutex::new(0)),
        }
    }

    pub(crate) fn dry_run_calls(&self) -> usize {
        *self.dry_run_calls.lock().expect("dry-run mutex")
    }

    pub(crate) fn execute_calls(&self) -> usize {
        *self.execute_calls.lock().expect("execute mutex")
    }
}

impl ActionAdapter for DryRunRaceAdapter {
    fn dry_run(
        &mut self,
        request: &ConfirmedExecutionRequest,
    ) -> Result<AdapterDryRun, AdapterError> {
        *self.dry_run_calls.lock().expect("dry-run mutex") += 1;

        let action = request.action().clone();
        let pool = self.pool.clone();
        std::thread::spawn(move || {
            runtime().block_on(async move {
                seed_resume_dry_run(&pool, &action, "race before", "race projected")
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

pub(crate) async fn seed_resume_confirmation(
    pool: &PgPool,
    action: &ConfirmedAction,
    summary_text: &str,
) -> Result<(), PostgresRepositoryError> {
    PostgresExecutionRecorder::new(pool.clone())
        .record_confirmation(
            action,
            1_748_260_000_000,
            &format!("op-{}", action.idempotency_key),
            &AuditEvent::confirmed_action(
                audit_context(
                    &resume_event_id(action, 1),
                    &resume_trace_id(action),
                    1,
                    1_748_260_001_000,
                    &action.actor_user_id,
                    &action.tenant_id,
                    &action.action_id,
                ),
                summary(summary_text),
            ),
            &outbox_envelope(
                &action.tenant_id,
                &resume_trace_id(action),
                1_748_260_002_000,
            ),
        )
        .await?;

    Ok(())
}

pub(crate) async fn seed_resume_dry_run(
    pool: &PgPool,
    action: &ConfirmedAction,
    before_summary: &str,
    after_summary: &str,
) -> Result<(), PostgresRepositoryError> {
    PostgresExecutionRecorder::new(pool.clone())
        .record_dry_run(
            &action.tenant_id,
            &action.idempotency_key,
            1_748_260_003_000,
            &AuditEvent::dry_run(
                audit_context(
                    &resume_event_id(action, 2),
                    &resume_trace_id(action),
                    2,
                    1_748_260_003_000,
                    &action.actor_user_id,
                    &action.tenant_id,
                    &action.action_id,
                ),
                Some(summary(before_summary)),
                Some(summary(after_summary)),
            ),
            &outbox_envelope(
                &action.tenant_id,
                &resume_trace_id(action),
                1_748_260_004_000,
            ),
        )
        .await?;

    Ok(())
}

pub(crate) fn resume_trace_id(action: &ConfirmedAction) -> String {
    format!("trace-{}-{}", action.tenant_id, action.idempotency_key)
}

pub(crate) async fn assert_persisted_audit_event_types(
    pool: &PgPool,
    tenant_id: &str,
    trace_id: &str,
    expected: &[AuditEventType],
) -> Result<(), PostgresRepositoryError> {
    let persisted = PostgresAuditEventRepository::new(pool.clone())
        .find_by_tenant_and_trace_id(tenant_id, trace_id)
        .await?;
    assert_eq!(
        persisted
            .iter()
            .map(|event| event.event_type.clone())
            .collect::<Vec<_>>(),
        expected
    );

    Ok(())
}

fn resume_event_id(action: &ConfirmedAction, sequence: u64) -> String {
    format!("{}-evt-{sequence}", resume_trace_id(action))
}

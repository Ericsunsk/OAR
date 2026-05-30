use super::harness::*;
use oar_core::action::execution_request::ConfirmedExecutionDecision;
use oar_core::action::postgres_execution_worker::{
    PostgresConfirmedActionDrainConfig, PostgresConfirmedActionWorker,
};
use serde_json::Value;

const EXECUTION_QUEUE_HASH: &str =
    "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

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

fn execution_payload(objective_id: &str, kr_id: &str, progress_delta: i64) -> Value {
    json!({
        "target": {
            "objective_id": objective_id,
            "kr_id": kr_id
        },
        "mutation": {
            "progress_delta": progress_delta,
            "note": "weekly check-in"
        }
    })
}

fn execution_evidence_item(id: &str, source_id: &str) -> EvidenceItem {
    EvidenceItem::new(
        EvidenceId(id.to_string()),
        "execution evidence",
        EvidenceRef::new(EvidenceSourceKind::OkrProgress, source_id, None)
            .expect("evidence reference should be valid"),
        EXECUTION_QUEUE_HASH,
        EvidenceVisibilityScope::Tenant,
        UNIX_EPOCH + std::time::Duration::from_millis(1_748_260_000_000),
        UNIX_EPOCH + std::time::Duration::from_millis(1_748_260_001_000),
    )
    .expect("evidence item should be valid")
}

fn proposed_execution_action(
    tenant_id: &str,
    user_id: &str,
    proposed_action_id: &str,
    evidence_id: &str,
    suggested_payload: Value,
) -> ProposedAction {
    let mut action = ProposedAction::draft(
        ProposedActionId(proposed_action_id.to_string()),
        TenantId(tenant_id.to_string()),
        WorkspaceUserId(user_id.to_string()),
        None,
        None,
        1,
        ProposedActionKind::UpdateKrProgress,
        RiskSeverity::High,
        vec![evidence_id.to_string()],
        suggested_payload,
    )
    .expect("proposed action should be valid");
    action.publish().expect("proposed action should publish");
    action
}

struct ExecutionQueueSeed<'a> {
    tenant_id: &'a str,
    user_id: &'a str,
    proposed_action_id: &'a str,
    confirmed_action_id: &'a str,
    idempotency_key: &'a str,
    operation_id: &'a str,
    decision: ProposedActionDecision,
    suggested_payload: Value,
}

async fn seed_confirmed_execution_queue_item(
    pool: &PgPool,
    seed: ExecutionQueueSeed<'_>,
) -> Result<ConfirmedAction, Box<dyn std::error::Error + Send + Sync>> {
    let review_repository = PostgresReviewInboxRepository::new(pool.clone());
    let evidence_id = format!("evidence_{}", seed.proposed_action_id);
    review_repository
        .insert_evidence_item(
            seed.tenant_id,
            &execution_evidence_item(&evidence_id, seed.proposed_action_id),
        )
        .await?;
    let proposed_action = proposed_execution_action(
        seed.tenant_id,
        seed.user_id,
        seed.proposed_action_id,
        &evidence_id,
        seed.suggested_payload,
    );
    review_repository
        .insert_proposed_action(
            &proposed_action,
            Some(UNIX_EPOCH + std::time::Duration::from_millis(1_748_260_002_000)),
        )
        .await?;
    review_repository
        .insert_proposed_action_evidence_ref(
            seed.tenant_id,
            seed.proposed_action_id,
            1,
            &evidence_id,
        )
        .await?;

    let confirmed = confirmed_action(
        seed.confirmed_action_id,
        seed.tenant_id,
        seed.user_id,
        seed.idempotency_key,
    );
    PostgresOperationLedgerRepository::new(pool.clone())
        .submit_confirmed_action(&confirmed, 1_748_260_003_000, seed.operation_id)
        .await?;

    let decision_id = format!("decision_{}", seed.proposed_action_id);
    review_repository
        .insert_proposed_action_decision(InsertProposedActionDecisionRequest {
            id: &decision_id,
            tenant_id: seed.tenant_id,
            proposed_action_id: seed.proposed_action_id,
            proposed_action_version: 1,
            actor_user_id: seed.user_id,
            decision: &seed.decision,
            confirmed_action_id: Some(&confirmed.action_id),
            decided_at: UNIX_EPOCH + std::time::Duration::from_millis(1_748_260_004_000),
        })
        .await?;

    Ok(confirmed)
}

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

#[test]
fn postgres_live_confirmed_action_queue_lists_only_ready_tenant_work() {
    run_live_postgres_test("confirmed_action_queue_ready", |pool| async move {
        seed_user(&pool, "tenant_queue_ready", "user_queue_ready").await?;
        seed_user(&pool, "tenant_queue_other", "user_queue_other").await?;

        let repository = PostgresOperationLedgerRepository::new(pool.clone());
        let ready_payload = execution_payload("objective_queue_ready", "kr_queue_ready", 12);
        let ready = seed_confirmed_execution_queue_item(
            &pool,
            ExecutionQueueSeed {
                tenant_id: "tenant_queue_ready",
                user_id: "user_queue_ready",
                proposed_action_id: "proposed_queue_ready",
                confirmed_action_id: "action_queue_ready",
                idempotency_key: "idem_queue_ready",
                operation_id: "op_queue_ready",
                decision: ProposedActionDecision::Confirm,
                suggested_payload: ready_payload.clone(),
            },
        )
        .await?;
        seed_confirmed_execution_queue_item(
            &pool,
            ExecutionQueueSeed {
                tenant_id: "tenant_queue_ready",
                user_id: "user_queue_ready",
                proposed_action_id: "proposed_queue_executing",
                confirmed_action_id: "action_queue_executing",
                idempotency_key: "idem_queue_executing",
                operation_id: "op_queue_executing",
                decision: ProposedActionDecision::Confirm,
                suggested_payload: execution_payload(
                    "objective_queue_executing",
                    "kr_queue_executing",
                    3,
                ),
            },
        )
        .await?;
        let _other_tenant = seed_confirmed_execution_queue_item(
            &pool,
            ExecutionQueueSeed {
                tenant_id: "tenant_queue_other",
                user_id: "user_queue_other",
                proposed_action_id: "proposed_queue_other",
                confirmed_action_id: "action_queue_other",
                idempotency_key: "idem_queue_other",
                operation_id: "op_queue_other",
                decision: ProposedActionDecision::Confirm,
                suggested_payload: execution_payload("objective_queue_other", "kr_queue_other", 7),
            },
        )
        .await?;
        repository
            .mark_executing(
                "tenant_queue_ready",
                "idem_queue_executing",
                1_748_260_003_000,
            )
            .await
            .map_err(|error| format!("mark_executing failed: {error:?}"))?;

        let pending = repository
            .list_confirmed_actions_ready_for_execution("tenant_queue_ready", 10)
            .await?;

        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].request.confirmed_action, ready);
        assert_eq!(
            pending[0].request.proposed_action_id,
            "proposed_queue_ready"
        );
        assert_eq!(pending[0].request.proposed_action_version, 1);
        assert_eq!(
            pending[0].request.action_kind,
            ProposedActionKind::UpdateKrProgress
        );
        assert_eq!(
            pending[0].request.evidence_ids,
            vec!["evidence_proposed_queue_ready".to_string()]
        );
        assert_eq!(pending[0].request.effective_payload, ready_payload);
        assert_eq!(
            pending[0].request.decision,
            ConfirmedExecutionDecision::Confirm
        );
        assert_eq!(pending[0].operation.operation_id, "op_queue_ready");
        assert_eq!(pending[0].operation.status, ActionStatus::Confirmed);

        Ok(())
    });
}

#[test]
fn postgres_live_confirmed_action_queue_uses_edited_payload_for_edit_then_confirm() {
    run_live_postgres_test("confirmed_action_queue_edit_payload", |pool| async move {
        seed_user(&pool, "tenant_queue_edit", "user_queue_edit").await?;

        let repository = PostgresOperationLedgerRepository::new(pool.clone());
        let suggested_payload = execution_payload("objective_queue_original", "kr_queue_edit", 1);
        let edited_payload = execution_payload("objective_queue_edited", "kr_queue_edit", 9);
        let confirmed = seed_confirmed_execution_queue_item(
            &pool,
            ExecutionQueueSeed {
                tenant_id: "tenant_queue_edit",
                user_id: "user_queue_edit",
                proposed_action_id: "proposed_queue_edit",
                confirmed_action_id: "action_queue_edit",
                idempotency_key: "idem_queue_edit",
                operation_id: "op_queue_edit",
                decision: ProposedActionDecision::EditThenConfirm {
                    edited_payload: edited_payload.clone(),
                },
                suggested_payload,
            },
        )
        .await?;

        let pending = repository
            .list_confirmed_actions_ready_for_execution("tenant_queue_edit", 10)
            .await?;

        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].request.confirmed_action, confirmed);
        assert_eq!(
            pending[0].request.decision,
            ConfirmedExecutionDecision::EditThenConfirm
        );
        assert_eq!(pending[0].request.effective_payload, edited_payload);

        Ok(())
    });
}

#[test]
fn postgres_live_confirmed_action_worker_drains_ready_work_through_executor() {
    run_live_postgres_test("confirmed_action_worker_drain", |pool| async move {
        seed_user(&pool, "tenant_worker_drain", "user_worker_drain").await?;

        let repository = PostgresOperationLedgerRepository::new(pool.clone());
        seed_confirmed_execution_queue_item(
            &pool,
            ExecutionQueueSeed {
                tenant_id: "tenant_worker_drain",
                user_id: "user_worker_drain",
                proposed_action_id: "proposed_worker_drain",
                confirmed_action_id: "action_worker_drain",
                idempotency_key: "idem_worker_drain",
                operation_id: "op_worker_drain",
                decision: ProposedActionDecision::Confirm,
                suggested_payload: execution_payload(
                    "objective_worker_drain",
                    "kr_worker_drain",
                    4,
                ),
            },
        )
        .await?;

        let adapter = LiveMockAdapter::succeeding();
        let executor = postgres_action_executor(pool.clone(), adapter.clone());
        let mut worker = PostgresConfirmedActionWorker::new(
            repository.clone(),
            executor,
            PostgresConfirmedActionDrainConfig::new("tenant_worker_drain", 10),
        );

        let report = worker.drain_once().await?;

        assert_eq!(report.selected, 1);
        assert_eq!(report.attempted, 1);
        assert_eq!(report.succeeded, 1);
        assert_eq!(report.failed, 0);
        assert_eq!(report.duplicate, 0);
        assert_eq!(report.execution_errors, 0);
        assert_eq!(adapter.dry_run_calls(), 1);
        assert_eq!(adapter.execute_calls(), 1);

        let operation = repository
            .get_by_idempotency_key("tenant_worker_drain", "idem_worker_drain")
            .await?
            .expect("operation should exist");
        assert_eq!(operation.status, ActionStatus::Succeeded);

        let second = worker.drain_once().await?;
        assert_eq!(second.selected, 0);
        assert_eq!(adapter.execute_calls(), 1);

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

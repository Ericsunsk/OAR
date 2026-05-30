use super::*;
use oar_core::action::execution_request::ConfirmedExecutionDecision;
use oar_core::action::postgres_execution_worker::{
    PostgresConfirmedActionDrainConfig, PostgresConfirmedActionWorker,
};

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

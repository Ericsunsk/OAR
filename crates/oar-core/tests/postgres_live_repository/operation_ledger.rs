use super::harness::*;

#[test]
fn postgres_repository_rejects_unconfirmed_action_before_db_access() {
    runtime().block_on(async {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://localhost/oar_unreachable")
            .expect("lazy pool should parse static database url");
        let repository = PostgresOperationLedgerRepository::new(pool);
        let proposed = ConfirmedAction::proposed("action", "tenant", "user", "idem");

        let error = repository
            .submit_confirmed_action(&proposed, 0, "op")
            .await
            .expect_err("proposed actions should be rejected before database access");

        assert!(error
            .to_string()
            .contains("action must be confirmed before persistence"));
    });
}

#[test]
fn postgres_repository_transition_reports_repository_failure_for_infra_errors() {
    runtime().block_on(async {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://localhost/oar_unreachable")
            .expect("lazy pool should parse static database url");
        let repository = PostgresOperationLedgerRepository::new(pool);

        let error = repository
            .mark_executing("tenant", "idem", 0)
            .await
            .expect_err("unreachable postgres should surface as repository failure");

        assert!(
            matches!(error, LedgerError::RepositoryFailure(_)),
            "infra errors must not be reported as unknown idempotency keys: {error:?}"
        );
    });
}

#[test]
fn postgres_live_operation_repository_preserves_idempotent_transitions() {
    run_live_postgres_test("operation_repository", |pool| async move {
        seed_user(&pool, "tenant_live", "user_live").await?;

        let repository = PostgresOperationLedgerRepository::new(pool.clone());
        let action = confirmed_action("action_live_1", "tenant_live", "user_live", "idem_live");

        let first = repository
            .submit_confirmed_action(&action, 1_748_250_000_000, "op_live_1")
            .await?;
        let second = repository
            .submit_confirmed_action(&action, 1_748_250_000_000, "op_live_2")
            .await?;

        let created = match first {
            SubmitResult::Created(record) => record,
            SubmitResult::Existing(_) => panic!("first submit should create an operation"),
        };
        let duplicate = match second {
            SubmitResult::Existing(record) => record,
            SubmitResult::Created(_) => panic!("duplicate submit should return existing operation"),
        };
        let same_operation_id_retry = repository
            .submit_confirmed_action(&action, 1_748_250_000_000, "op_live_1")
            .await?;
        let same_operation_id_duplicate = match same_operation_id_retry {
            SubmitResult::Existing(record) => record,
            SubmitResult::Created(_) => {
                panic!("duplicate submit should not be inferred from matching operation_id")
            }
        };

        assert_eq!(created.operation_id, "op_live_1");
        assert_eq!(duplicate.operation_id, created.operation_id);
        assert_eq!(
            same_operation_id_duplicate.operation_id,
            created.operation_id
        );
        assert_eq!(duplicate.status, ActionStatus::Confirmed);

        let executing = repository
            .mark_executing("tenant_live", "idem_live", 1_748_250_001_000)
            .await
            .map_err(|error| format!("mark_executing failed: {error:?}"))?;
        let duplicate_executing = repository
            .mark_executing("tenant_live", "idem_live", 1_748_250_002_000)
            .await
            .map_err(|error| format!("duplicate mark_executing failed: {error:?}"))?;
        assert_eq!(executing.operation_id, duplicate_executing.operation_id);
        assert_eq!(duplicate_executing.status, ActionStatus::Executing);

        let succeeded = repository
            .mark_succeeded("tenant_live", "idem_live", 1_748_250_003_000)
            .await
            .map_err(|error| format!("mark_succeeded failed: {error:?}"))?;
        let duplicate_succeeded = repository
            .mark_succeeded("tenant_live", "idem_live", 1_748_250_004_000)
            .await
            .map_err(|error| format!("duplicate mark_succeeded failed: {error:?}"))?;
        assert_eq!(succeeded.operation_id, duplicate_succeeded.operation_id);
        assert_eq!(duplicate_succeeded.status, ActionStatus::Succeeded);

        let invalid_retry = repository
            .mark_executing("tenant_live", "idem_live", 1_748_250_005_000)
            .await;
        assert_eq!(
            invalid_retry,
            Err(LedgerError::InvalidTransition {
                from: ActionStatus::Succeeded,
                to: ActionStatus::Executing,
            })
        );

        let missing = repository
            .mark_executing("tenant_live", "missing_idem", 1_748_250_006_000)
            .await;
        assert_eq!(
            missing,
            Err(LedgerError::UnknownIdempotencyKey(
                "missing_idem".to_string()
            ))
        );

        Ok(())
    });
}

#[test]
fn postgres_live_operation_lookup_is_tenant_scoped() {
    run_live_postgres_test("operation_tenant_scope", |pool| async move {
        seed_user(&pool, "tenant_a", "user_a").await?;
        seed_user(&pool, "tenant_b", "user_b").await?;

        let repository = PostgresOperationLedgerRepository::new(pool);
        let action_a = confirmed_action("action_a", "tenant_a", "user_a", "shared_idem");
        let action_b = confirmed_action("action_b", "tenant_b", "user_b", "shared_idem");

        repository
            .submit_confirmed_action(&action_a, 1_748_250_000_000, "op_a")
            .await?;
        repository
            .submit_confirmed_action(&action_b, 1_748_250_000_000, "op_b")
            .await?;

        let record_a = repository
            .get_by_idempotency_key("tenant_a", "shared_idem")
            .await?
            .expect("tenant A record should exist");
        let record_b = repository
            .get_by_idempotency_key("tenant_b", "shared_idem")
            .await?
            .expect("tenant B record should exist");

        assert_eq!(record_a.operation_id, "op_a");
        assert_eq!(record_a.action_id, "action_a");
        assert_eq!(record_b.operation_id, "op_b");
        assert_eq!(record_b.action_id, "action_b");

        Ok(())
    });
}

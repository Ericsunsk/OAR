use super::harness::*;

fn assert_safe_stage_error(value: &str) {
    let lowered = value.to_ascii_lowercase();
    for marker in [
        "access_token",
        "refresh_token",
        "authorization_code",
        "authorization:",
        "bearer ",
        "stdout",
        "stderr",
        "encrypted",
        "fingerprint",
        "oauth_grant",
        "tok_",
        "rt_fake",
        "at_fake",
    ] {
        assert!(
            !lowered.contains(marker),
            "tenant maintenance stage error leaked sensitive marker: {marker}"
        );
    }
}

#[test]
fn postgres_live_tenant_maintenance_run_once_executes_sweep_then_outbox_drain() {
    run_live_postgres_test("tenant_maintenance_run_once_success", |pool| async move {
        let due_before_ms = 1_748_700_000_000u64;
        let now_ms = 1_748_700_500_000u64;
        let now = UNIX_EPOCH + std::time::Duration::from_millis(now_ms);

        seed_user(
            &pool,
            "tenant_maintenance_success",
            "user_maintenance_success",
        )
        .await?;
        seed_identity(
            &pool,
            "tenant_maintenance_success",
            "identity_maintenance_success",
        )
        .await?;

        let scheduler = PostgresSchedulerJobRepository::new(pool.clone());
        scheduler
            .upsert_job(
                "job_maintenance_success",
                "tenant_maintenance_success",
                SchedulerJobKind::TokenRefreshSweep,
                due_before_ms,
            )
            .await?;

        let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
        let mut due = encrypted_token_grant_record(
            "tenant_maintenance_success",
            "grant_maintenance_due",
            "identity_maintenance_success",
            TokenGrantState::NeedsRefresh,
            "fp-maintenance-old",
        );
        due.expires_at_ms = Some(due_before_ms - 1_000);
        grant_repo.upsert_encrypted_grant(&due).await?;

        let audit_repo = PostgresAuditEventRepository::new(pool.clone());
        audit_repo
            .enqueue_outbox(
                "tenant_maintenance_success",
                "audit-events",
                "trace_maintenance_outbox",
                &json!({ "kind": "maintenance_outbox" }),
                now_ms - 10_000,
            )
            .await?;

        let refresh_adapter = SequenceRefreshAdapter::new([RefreshOutcome::Success {
            rotated_material: EncryptedGrantMaterial {
                encrypted_primary: vec![0x31],
                encrypted_renewal: vec![0x41],
            },
            key_id: "key-maintenance-v2".to_string(),
            new_fingerprint: "fp-maintenance-new".to_string(),
            refreshed_at: now,
            expires_at: Some(UNIX_EPOCH + std::time::Duration::from_millis(1_748_800_000_000)),
        }]);
        let outbox_dispatcher = LiveOutboxDispatcher::new([AuditOutboxDelivery::Sent]);

        let mut ticks = vec![
            now_ms,
            now_ms + 1_000,
            now_ms + 1_500,
            now_ms + 1_800,
            now_ms + 2_000,
        ];
        ticks.reverse();
        let mut worker = PostgresTenantMaintenanceWorker::new(
            pool.clone(),
            refresh_adapter.clone(),
            outbox_dispatcher,
            move || ticks.pop().unwrap_or(now_ms + 9_000),
            PostgresTenantMaintenanceConfig {
                tenant_id: "tenant_maintenance_success".to_string(),
                lease_id: "lease_maintenance_success".to_string(),
                audit_stream: "audit-events".to_string(),
                scheduled_lease_ms: 10_000,
                scheduled_retry_delay_ms: 30_000,
                scheduled_next_run_delay_ms: 86_400_000,
                scheduled_backlog_next_run_delay_ms: 5_000,
                scheduled_due_before_ms: due_before_ms,
                scheduled_limit: 4,
                scheduled_audit_trace_id: "trace_maintenance_scheduled".to_string(),
                scheduled_audit_sequence_start: 401,
                scheduled_actor: actor("user_maintenance_success"),
                scheduled_workspace_id: None,
                outbox_batch_limit: 16,
                outbox_lease_ms: 15_000,
                outbox_retry_delay_ms: 60_000,
                outbox_max_attempts: 3,
            },
        );

        let report = worker.run_once().await?;
        let scheduled = report
            .scheduled_sweep
            .succeeded()
            .expect("scheduled sweep stage should succeed");
        let outbox = report
            .outbox_drain
            .succeeded()
            .expect("outbox stage should succeed");

        assert_eq!(scheduled.attempt.outcome, SchedulerJobOutcome::Succeeded);
        let sweep = scheduled
            .sweep
            .as_ref()
            .expect("scheduled sweep report should exist");
        assert_eq!(sweep.attempted_count, 1);
        assert_eq!(
            refresh_adapter.called_grant_ids(),
            vec!["grant_maintenance_due".to_string()]
        );

        assert_eq!(outbox.claimed, 1);
        assert_eq!(outbox.sent, 1);
        assert_eq!(outbox.retryable, 0);
        assert_eq!(outbox.failed, 0);

        let rotated = grant_repo
            .get_by_id("tenant_maintenance_success", "grant_maintenance_due")
            .await?
            .expect("grant should still exist");
        assert_eq!(rotated.state, TokenGrantState::Valid);
        assert_eq!(rotated.oauth_grant_fingerprint, "fp-maintenance-new");

        let outbox_status: String = sqlx::query_scalar(
            r#"
            SELECT status
            FROM audit_outbox
            WHERE tenant_id = $1
              AND aggregate_id = $2
            "#,
        )
        .bind("tenant_maintenance_success")
        .bind("trace_maintenance_outbox")
        .fetch_one(&pool)
        .await?;
        assert_eq!(outbox_status, "sent");

        Ok(())
    });
}

#[test]
fn postgres_live_tenant_maintenance_run_once_busy_scheduler_still_drains_outbox() {
    run_live_postgres_test("tenant_maintenance_busy_drain", |pool| async move {
        let due_before_ms = 1_748_710_000_000u64;
        let now_ms = 1_748_710_500_000u64;

        seed_user(&pool, "tenant_maintenance_busy", "user_maintenance_busy").await?;
        seed_identity(
            &pool,
            "tenant_maintenance_busy",
            "identity_maintenance_busy",
        )
        .await?;

        let scheduler = PostgresSchedulerJobRepository::new(pool.clone());
        scheduler
            .upsert_job(
                "job_maintenance_busy",
                "tenant_maintenance_busy",
                SchedulerJobKind::TokenRefreshSweep,
                due_before_ms,
            )
            .await?;
        let held = scheduler
            .try_acquire(
                "tenant_maintenance_busy",
                SchedulerJobKind::TokenRefreshSweep,
                now_ms,
                "lease_maintenance_busy_held",
                now_ms + 60_000,
            )
            .await?;
        assert!(matches!(held, SchedulerLeaseAcquire::Acquired(_)));

        let audit_repo = PostgresAuditEventRepository::new(pool.clone());
        audit_repo
            .enqueue_outbox(
                "tenant_maintenance_busy",
                "audit-events",
                "trace_maintenance_busy_outbox",
                &json!({ "kind": "maintenance_outbox_busy" }),
                now_ms - 1_000,
            )
            .await?;

        let refresh_adapter = SequenceRefreshAdapter::new([RefreshOutcome::TransientFailure {
            safe_error: "temporarily unavailable".to_string(),
        }]);
        let outbox_dispatcher = LiveOutboxDispatcher::new([AuditOutboxDelivery::Sent]);

        let mut ticks = vec![now_ms, now_ms + 1_000, now_ms + 2_000];
        ticks.reverse();
        let mut worker = PostgresTenantMaintenanceWorker::new(
            pool.clone(),
            refresh_adapter.clone(),
            outbox_dispatcher,
            move || ticks.pop().unwrap_or(now_ms + 9_000),
            PostgresTenantMaintenanceConfig {
                tenant_id: "tenant_maintenance_busy".to_string(),
                lease_id: "lease_maintenance_busy_runner".to_string(),
                audit_stream: "audit-events".to_string(),
                scheduled_lease_ms: 10_000,
                scheduled_retry_delay_ms: 30_000,
                scheduled_next_run_delay_ms: 86_400_000,
                scheduled_backlog_next_run_delay_ms: 5_000,
                scheduled_due_before_ms: due_before_ms,
                scheduled_limit: 4,
                scheduled_audit_trace_id: "trace_maintenance_busy_scheduled".to_string(),
                scheduled_audit_sequence_start: 501,
                scheduled_actor: actor("user_maintenance_busy"),
                scheduled_workspace_id: None,
                outbox_batch_limit: 16,
                outbox_lease_ms: 15_000,
                outbox_retry_delay_ms: 60_000,
                outbox_max_attempts: 3,
            },
        );

        let report = worker.run_once().await?;
        let scheduled = report
            .scheduled_sweep
            .succeeded()
            .expect("scheduled sweep stage should succeed");
        let outbox = report
            .outbox_drain
            .succeeded()
            .expect("outbox stage should succeed");

        assert_eq!(scheduled.attempt.outcome, SchedulerJobOutcome::SkippedBusy);
        assert!(scheduled.sweep.is_none());
        assert!(refresh_adapter.called_grant_ids().is_empty());

        assert_eq!(outbox.claimed, 1);
        assert_eq!(outbox.sent, 1);

        let outbox_status: String = sqlx::query_scalar(
            r#"
            SELECT status
            FROM audit_outbox
            WHERE tenant_id = $1
              AND aggregate_id = $2
            "#,
        )
        .bind("tenant_maintenance_busy")
        .bind("trace_maintenance_busy_outbox")
        .fetch_one(&pool)
        .await?;
        assert_eq!(outbox_status, "sent");

        Ok(())
    });
}

#[test]
fn postgres_live_tenant_maintenance_run_once_sweep_error_still_drains_outbox() {
    run_live_postgres_test("tenant_maintenance_sweep_error_drain", |pool| async move {
        let due_before_ms = 1_748_720_000_000u64;
        let now_ms = 1_748_720_500_000u64;

        seed_user(
            &pool,
            "tenant_maintenance_sweep_error",
            "user_maintenance_sweep_error",
        )
        .await?;
        seed_identity(
            &pool,
            "tenant_maintenance_sweep_error",
            "identity_maintenance_sweep_error",
        )
        .await?;

        let scheduler = PostgresSchedulerJobRepository::new(pool.clone());
        scheduler
            .upsert_job(
                "job_maintenance_sweep_error",
                "tenant_maintenance_sweep_error",
                SchedulerJobKind::TokenRefreshSweep,
                due_before_ms,
            )
            .await?;
        sqlx::query("DROP TABLE scheduler_jobs")
            .execute(&pool)
            .await?;

        let audit_repo = PostgresAuditEventRepository::new(pool.clone());
        audit_repo
            .enqueue_outbox(
                "tenant_maintenance_sweep_error",
                "audit-events",
                "trace_maintenance_sweep_error_outbox",
                &json!({ "kind": "maintenance_outbox_sweep_error" }),
                now_ms - 1_000,
            )
            .await?;

        let refresh_adapter = SequenceRefreshAdapter::new([RefreshOutcome::TransientFailure {
            safe_error: "temporarily unavailable".to_string(),
        }]);
        let outbox_dispatcher = LiveOutboxDispatcher::new([AuditOutboxDelivery::Sent]);

        let mut ticks = vec![now_ms, now_ms + 1_000, now_ms + 2_000];
        ticks.reverse();
        let mut worker = PostgresTenantMaintenanceWorker::new(
            pool.clone(),
            refresh_adapter.clone(),
            outbox_dispatcher,
            move || ticks.pop().unwrap_or(now_ms + 9_000),
            PostgresTenantMaintenanceConfig {
                tenant_id: "tenant_maintenance_sweep_error".to_string(),
                lease_id: "lease_maintenance_sweep_error_runner".to_string(),
                audit_stream: "audit-events".to_string(),
                scheduled_lease_ms: 10_000,
                scheduled_retry_delay_ms: 30_000,
                scheduled_next_run_delay_ms: 86_400_000,
                scheduled_backlog_next_run_delay_ms: 5_000,
                scheduled_due_before_ms: due_before_ms,
                scheduled_limit: 4,
                scheduled_audit_trace_id: "trace_maintenance_sweep_error_scheduled".to_string(),
                scheduled_audit_sequence_start: 601,
                scheduled_actor: actor("user_maintenance_sweep_error"),
                scheduled_workspace_id: None,
                outbox_batch_limit: 16,
                outbox_lease_ms: 15_000,
                outbox_retry_delay_ms: 60_000,
                outbox_max_attempts: 3,
            },
        );

        let report = worker.run_once().await?;
        let sweep_failure = report
            .scheduled_sweep
            .failed()
            .expect("scheduled sweep stage should fail");
        let outbox = report
            .outbox_drain
            .succeeded()
            .expect("outbox stage should still succeed");

        assert_eq!(
            sweep_failure.safe_error,
            "tenant_maintenance_stage_failed: postgres_query_failed"
        );
        assert_safe_stage_error(&sweep_failure.safe_error);

        assert_eq!(outbox.claimed, 1);
        assert_eq!(outbox.sent, 1);
        assert!(refresh_adapter.called_grant_ids().is_empty());

        let outbox_status: String = sqlx::query_scalar(
            r#"
            SELECT status
            FROM audit_outbox
            WHERE tenant_id = $1
              AND aggregate_id = $2
            "#,
        )
        .bind("tenant_maintenance_sweep_error")
        .bind("trace_maintenance_sweep_error_outbox")
        .fetch_one(&pool)
        .await?;
        assert_eq!(outbox_status, "sent");

        Ok(())
    });
}

#[test]
fn postgres_live_tenant_maintenance_run_once_outbox_error_reports_stage_failure() {
    run_live_postgres_test("tenant_maintenance_outbox_error", |pool| async move {
        let due_before_ms = 1_748_730_000_000u64;
        let now_ms = 1_748_730_500_000u64;

        seed_user(
            &pool,
            "tenant_maintenance_outbox_error",
            "user_maintenance_outbox_error",
        )
        .await?;
        seed_identity(
            &pool,
            "tenant_maintenance_outbox_error",
            "identity_maintenance_outbox_error",
        )
        .await?;

        let scheduler = PostgresSchedulerJobRepository::new(pool.clone());
        scheduler
            .upsert_job(
                "job_maintenance_outbox_error",
                "tenant_maintenance_outbox_error",
                SchedulerJobKind::TokenRefreshSweep,
                due_before_ms,
            )
            .await?;
        sqlx::query("DROP TABLE audit_outbox")
            .execute(&pool)
            .await?;

        let refresh_adapter = SequenceRefreshAdapter::new([RefreshOutcome::TransientFailure {
            safe_error: "temporarily unavailable".to_string(),
        }]);
        let outbox_dispatcher = LiveOutboxDispatcher::new([AuditOutboxDelivery::Sent]);

        let mut ticks = vec![now_ms, now_ms + 1_000, now_ms + 2_000];
        ticks.reverse();
        let mut worker = PostgresTenantMaintenanceWorker::new(
            pool.clone(),
            refresh_adapter.clone(),
            outbox_dispatcher,
            move || ticks.pop().unwrap_or(now_ms + 9_000),
            PostgresTenantMaintenanceConfig {
                tenant_id: "tenant_maintenance_outbox_error".to_string(),
                lease_id: "lease_maintenance_outbox_error_runner".to_string(),
                audit_stream: "audit-events".to_string(),
                scheduled_lease_ms: 10_000,
                scheduled_retry_delay_ms: 30_000,
                scheduled_next_run_delay_ms: 86_400_000,
                scheduled_backlog_next_run_delay_ms: 5_000,
                scheduled_due_before_ms: due_before_ms,
                scheduled_limit: 4,
                scheduled_audit_trace_id: "trace_maintenance_outbox_error_scheduled".to_string(),
                scheduled_audit_sequence_start: 701,
                scheduled_actor: actor("user_maintenance_outbox_error"),
                scheduled_workspace_id: None,
                outbox_batch_limit: 16,
                outbox_lease_ms: 15_000,
                outbox_retry_delay_ms: 60_000,
                outbox_max_attempts: 3,
            },
        );

        let report = worker.run_once().await?;
        let scheduled = report
            .scheduled_sweep
            .succeeded()
            .expect("scheduled sweep stage should still succeed");
        let outbox_failure = report
            .outbox_drain
            .failed()
            .expect("outbox drain stage should fail");

        assert_eq!(scheduled.attempt.outcome, SchedulerJobOutcome::Noop);
        assert!(scheduled.sweep.is_some());
        assert!(refresh_adapter.called_grant_ids().is_empty());
        assert_eq!(
            outbox_failure.safe_error,
            "tenant_maintenance_stage_failed: postgres_query_failed"
        );
        assert_safe_stage_error(&outbox_failure.safe_error);

        Ok(())
    });
}

#[test]
fn postgres_live_tenant_maintenance_run_once_outbox_delivery_counts_retryable_failed_and_exhausted()
{
    run_live_postgres_test(
        "tenant_maintenance_outbox_delivery_counts",
        |pool| async move {
            let due_before_ms = 1_748_740_000_000u64;
            let now_ms = 1_748_740_500_000u64;

            seed_user(
                &pool,
                "tenant_maintenance_outbox_counts",
                "user_maintenance_outbox_counts",
            )
            .await?;
            seed_identity(
                &pool,
                "tenant_maintenance_outbox_counts",
                "identity_maintenance_outbox_counts",
            )
            .await?;

            let audit_repo = PostgresAuditEventRepository::new(pool.clone());
            audit_repo
                .enqueue_outbox(
                    "tenant_maintenance_outbox_counts",
                    "audit-events",
                    "trace_maintenance_outbox_counts_retryable",
                    &json!({ "kind": "maintenance_outbox_counts_retryable" }),
                    now_ms - 10_000,
                )
                .await?;
            audit_repo
                .enqueue_outbox(
                    "tenant_maintenance_outbox_counts",
                    "audit-events",
                    "trace_maintenance_outbox_counts_failed",
                    &json!({ "kind": "maintenance_outbox_counts_failed" }),
                    now_ms - 10_000,
                )
                .await?;
            audit_repo
                .enqueue_outbox(
                    "tenant_maintenance_outbox_counts",
                    "audit-events",
                    "trace_maintenance_outbox_counts_exhausted",
                    &json!({ "kind": "maintenance_outbox_counts_exhausted" }),
                    now_ms - 10_000,
                )
                .await?;

            sqlx::query(
                r#"
            UPDATE audit_outbox
            SET attempt_count = 3
            WHERE tenant_id = $1
              AND aggregate_id = $2
            "#,
            )
            .bind("tenant_maintenance_outbox_counts")
            .bind("trace_maintenance_outbox_counts_exhausted")
            .execute(&pool)
            .await?;

            let refresh_adapter = SequenceRefreshAdapter::new([RefreshOutcome::TransientFailure {
                safe_error: "temporarily unavailable".to_string(),
            }]);
            let outbox_dispatcher = LiveOutboxDispatcher::new([
                AuditOutboxDelivery::Retryable,
                AuditOutboxDelivery::Failed,
                AuditOutboxDelivery::Retryable,
            ]);

            let mut ticks = vec![now_ms, now_ms + 1_000, now_ms + 2_000, now_ms + 3_000];
            ticks.reverse();
            let mut worker = PostgresTenantMaintenanceWorker::new(
                pool.clone(),
                refresh_adapter,
                outbox_dispatcher,
                move || ticks.pop().unwrap_or(now_ms + 9_000),
                PostgresTenantMaintenanceConfig {
                    tenant_id: "tenant_maintenance_outbox_counts".to_string(),
                    lease_id: "lease_maintenance_outbox_counts_runner".to_string(),
                    audit_stream: "audit-events".to_string(),
                    scheduled_lease_ms: 10_000,
                    scheduled_retry_delay_ms: 30_000,
                    scheduled_next_run_delay_ms: 86_400_000,
                    scheduled_backlog_next_run_delay_ms: 5_000,
                    scheduled_due_before_ms: due_before_ms,
                    scheduled_limit: 4,
                    scheduled_audit_trace_id: "trace_maintenance_outbox_counts_scheduled"
                        .to_string(),
                    scheduled_audit_sequence_start: 801,
                    scheduled_actor: actor("user_maintenance_outbox_counts"),
                    scheduled_workspace_id: None,
                    outbox_batch_limit: 16,
                    outbox_lease_ms: 15_000,
                    outbox_retry_delay_ms: 60_000,
                    outbox_max_attempts: 3,
                },
            );

            let report = worker.run_once().await?;
            let scheduled = report
                .scheduled_sweep
                .succeeded()
                .expect("scheduled sweep stage should succeed");
            let outbox = report
                .outbox_drain
                .succeeded()
                .expect("outbox stage should succeed");

            assert_eq!(scheduled.attempt.outcome, SchedulerJobOutcome::Noop);
            assert_eq!(outbox.claimed, 3);
            assert_eq!(outbox.sent, 0);
            assert_eq!(outbox.retryable, 1);
            assert_eq!(outbox.failed, 2);
            assert_eq!(outbox.exhausted, 1);
            assert_eq!(outbox.stale, 0);

            let retryable_status: String = sqlx::query_scalar(
                r#"
            SELECT status
            FROM audit_outbox
            WHERE tenant_id = $1
              AND aggregate_id = $2
            "#,
            )
            .bind("tenant_maintenance_outbox_counts")
            .bind("trace_maintenance_outbox_counts_retryable")
            .fetch_one(&pool)
            .await?;
            assert_eq!(retryable_status, "pending");

            let failed_status: String = sqlx::query_scalar(
                r#"
            SELECT status
            FROM audit_outbox
            WHERE tenant_id = $1
              AND aggregate_id = $2
            "#,
            )
            .bind("tenant_maintenance_outbox_counts")
            .bind("trace_maintenance_outbox_counts_failed")
            .fetch_one(&pool)
            .await?;
            assert_eq!(failed_status, "failed");

            let exhausted_status: String = sqlx::query_scalar(
                r#"
            SELECT status
            FROM audit_outbox
            WHERE tenant_id = $1
              AND aggregate_id = $2
            "#,
            )
            .bind("tenant_maintenance_outbox_counts")
            .bind("trace_maintenance_outbox_counts_exhausted")
            .fetch_one(&pool)
            .await?;
            assert_eq!(exhausted_status, "failed");

            Ok(())
        },
    );
}

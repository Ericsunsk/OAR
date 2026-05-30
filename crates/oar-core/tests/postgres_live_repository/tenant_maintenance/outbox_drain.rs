use super::*;

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

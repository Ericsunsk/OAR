use super::*;

#[test]
fn postgres_live_token_refresh_scheduled_sweep_skips_busy_without_adapter_or_audit() {
    run_live_postgres_test("token_refresh_scheduled_sweep_busy", |pool| async move {
        let due_before_ms = 1_748_580_000_000u64;
        let now_ms = 1_748_580_500_000u64;

        seed_user(&pool, "tenant_tr_scheduled_busy", "user_tr_scheduled_busy").await?;
        seed_identity(
            &pool,
            "tenant_tr_scheduled_busy",
            "identity_tr_scheduled_busy",
        )
        .await?;

        let scheduler = PostgresSchedulerJobRepository::new(pool.clone());
        scheduler
            .upsert_job(
                "job_tr_scheduled_busy",
                "tenant_tr_scheduled_busy",
                SchedulerJobKind::TokenRefreshSweep,
                due_before_ms,
            )
            .await?;
        let held = scheduler
            .try_acquire(
                "tenant_tr_scheduled_busy",
                SchedulerJobKind::TokenRefreshSweep,
                now_ms,
                "lease_tr_scheduled_held",
                now_ms + 60_000,
            )
            .await?;
        assert!(matches!(held, SchedulerLeaseAcquire::Acquired(_)));

        let adapter = SequenceRefreshAdapter::new([RefreshOutcome::TransientFailure {
            safe_error: "temporarily unavailable".to_string(),
        }]);
        let mut ticks = vec![now_ms + 1_000, now_ms + 2_000];
        ticks.reverse();
        let mut scheduled = PostgresTokenRefreshScheduledSweep::new(
            scheduler,
            PostgresTokenRefreshSweep::new(pool.clone(), adapter.clone()),
            move || ticks.pop().unwrap_or(now_ms + 9_000),
            TokenRefreshScheduledSweepConfig {
                tenant_id: "tenant_tr_scheduled_busy".to_string(),
                lease_id: "lease_tr_scheduled_loser".to_string(),
                lease_ms: 10_000,
                retry_delay_ms: 30_000,
                next_run_delay_ms: 86_400_000,
                backlog_next_run_delay_ms: 5_000,
                due_before_ms,
                limit: 4,
                audit_trace_id: "trace_tr_scheduled_busy".to_string(),
                audit_sequence_start: 91,
                actor: actor("user_tr_scheduled_busy"),
                workspace_id: None,
            },
        );

        let report = scheduled.run_once().await?;

        assert_eq!(report.attempt.outcome, SchedulerJobOutcome::SkippedBusy);
        assert!(report.sweep.is_none());
        assert!(adapter.called_grant_ids().is_empty());
        let events = PostgresAuditEventRepository::new(pool.clone())
            .find_by_tenant_and_trace_id("tenant_tr_scheduled_busy", "trace_tr_scheduled_busy")
            .await?;
        assert!(events.is_empty());

        Ok(())
    });
}

#[test]
fn postgres_live_token_refresh_scheduled_sweep_skips_not_due() {
    run_live_postgres_test("token_refresh_scheduled_sweep_not_due", |pool| async move {
        let due_before_ms = 1_748_590_000_000u64;
        let now_ms = 1_748_590_500_000u64;

        seed_user(
            &pool,
            "tenant_tr_scheduled_not_due",
            "user_tr_scheduled_not_due",
        )
        .await?;
        seed_identity(
            &pool,
            "tenant_tr_scheduled_not_due",
            "identity_tr_scheduled_not_due",
        )
        .await?;

        let scheduler = PostgresSchedulerJobRepository::new(pool.clone());
        scheduler
            .upsert_job(
                "job_tr_scheduled_not_due",
                "tenant_tr_scheduled_not_due",
                SchedulerJobKind::TokenRefreshSweep,
                now_ms + 60_000,
            )
            .await?;

        let adapter = SequenceRefreshAdapter::new([RefreshOutcome::TransientFailure {
            safe_error: "temporarily unavailable".to_string(),
        }]);
        let mut ticks = vec![now_ms, now_ms + 1_000];
        ticks.reverse();
        let mut scheduled = PostgresTokenRefreshScheduledSweep::new(
            scheduler,
            PostgresTokenRefreshSweep::new(pool.clone(), adapter.clone()),
            move || ticks.pop().unwrap_or(now_ms + 9_000),
            TokenRefreshScheduledSweepConfig {
                tenant_id: "tenant_tr_scheduled_not_due".to_string(),
                lease_id: "lease_tr_scheduled_not_due".to_string(),
                lease_ms: 10_000,
                retry_delay_ms: 30_000,
                next_run_delay_ms: 86_400_000,
                backlog_next_run_delay_ms: 5_000,
                due_before_ms,
                limit: 4,
                audit_trace_id: "trace_tr_scheduled_not_due".to_string(),
                audit_sequence_start: 101,
                actor: actor("user_tr_scheduled_not_due"),
                workspace_id: None,
            },
        );

        let report = scheduled.run_once().await?;

        assert_eq!(report.attempt.outcome, SchedulerJobOutcome::SkippedNotDue);
        assert!(report.sweep.is_none());
        assert!(adapter.called_grant_ids().is_empty());

        Ok(())
    });
}

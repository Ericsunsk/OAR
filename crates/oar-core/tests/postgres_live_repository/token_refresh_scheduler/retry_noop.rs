use super::*;

#[test]
fn postgres_live_token_refresh_scheduled_sweep_retry_records_safe_failure() {
    run_live_postgres_test("token_refresh_scheduled_sweep_retry", |pool| async move {
        let due_before_ms = 1_748_600_000_000u64;
        let now_ms = 1_748_600_500_000u64;

        seed_user(
            &pool,
            "tenant_tr_scheduled_retry",
            "user_tr_scheduled_retry",
        )
        .await?;

        let scheduler = PostgresSchedulerJobRepository::new(pool.clone());
        scheduler
            .upsert_job(
                "job_tr_scheduled_retry",
                "tenant_tr_scheduled_retry",
                SchedulerJobKind::TokenRefreshSweep,
                due_before_ms,
            )
            .await?;

        let adapter = SequenceRefreshAdapter::new([RefreshOutcome::TransientFailure {
            safe_error: "temporarily unavailable".to_string(),
        }]);
        let mut ticks = vec![now_ms, now_ms + 5_000];
        ticks.reverse();
        let mut scheduled = PostgresTokenRefreshScheduledSweep::new(
            scheduler.clone(),
            PostgresTokenRefreshSweep::new(pool.clone(), adapter.clone()),
            move || ticks.pop().unwrap_or(now_ms + 9_000),
            TokenRefreshScheduledSweepConfig {
                tenant_id: "tenant_tr_scheduled_retry".to_string(),
                lease_id: "lease_tr_scheduled_retry".to_string(),
                lease_ms: 10_000,
                retry_delay_ms: 30_000,
                next_run_delay_ms: 86_400_000,
                backlog_next_run_delay_ms: 5_000,
                due_before_ms,
                limit: 4,
                audit_trace_id: "trace_tr_scheduled_retry".to_string(),
                audit_sequence_start: 111,
                actor: actor("user_tr_scheduled_retry"),
                workspace_id: None,
            },
        );

        let report = scheduled.run_once().await?;

        assert_eq!(report.attempt.outcome, SchedulerJobOutcome::FailedSafe);
        assert_eq!(
            report.attempt.safe_error_code.as_deref(),
            Some("token_refresh_sweep_failed")
        );
        assert!(report.sweep.is_none());
        assert!(adapter.called_grant_ids().is_empty());

        let job = scheduler
            .get_job(
                "tenant_tr_scheduled_retry",
                SchedulerJobKind::TokenRefreshSweep,
            )
            .await?
            .expect("scheduled retry job should exist");
        assert_eq!(job.status, SchedulerJobStatus::Pending);
        assert_eq!(job.next_run_at_ms, now_ms + 5_000 + 30_000);
        assert_eq!(
            job.last_safe_error_code.as_deref(),
            Some("token_refresh_sweep_failed")
        );

        let payload = format!("{report:?}");
        assert_no_auth_refresh_sensitive_payload(&payload);
        assert!(!payload.contains("fingerprint"));
        assert!(!payload.contains("encrypted"));

        Ok(())
    });
}

#[test]
fn postgres_live_token_refresh_scheduled_sweep_noop_completes_and_reschedules() {
    run_live_postgres_test("token_refresh_scheduled_sweep_noop", |pool| async move {
        let due_before_ms = 1_748_610_000_000u64;
        let now_ms = 1_748_610_500_000u64;

        seed_user(&pool, "tenant_tr_scheduled_noop", "user_tr_scheduled_noop").await?;

        let scheduler = PostgresSchedulerJobRepository::new(pool.clone());
        scheduler
            .upsert_job(
                "job_tr_scheduled_noop",
                "tenant_tr_scheduled_noop",
                SchedulerJobKind::TokenRefreshSweep,
                due_before_ms,
            )
            .await?;

        let adapter = SequenceRefreshAdapter::new([RefreshOutcome::TransientFailure {
            safe_error: "temporarily unavailable".to_string(),
        }]);
        let mut ticks = vec![now_ms, now_ms + 3_000];
        ticks.reverse();
        let mut scheduled = PostgresTokenRefreshScheduledSweep::new(
            scheduler.clone(),
            PostgresTokenRefreshSweep::new(pool.clone(), adapter.clone()),
            move || ticks.pop().unwrap_or(now_ms + 9_000),
            TokenRefreshScheduledSweepConfig {
                tenant_id: "tenant_tr_scheduled_noop".to_string(),
                lease_id: "lease_tr_scheduled_noop".to_string(),
                lease_ms: 10_000,
                retry_delay_ms: 30_000,
                next_run_delay_ms: 86_400_000,
                backlog_next_run_delay_ms: 5_000,
                due_before_ms,
                limit: 4,
                audit_trace_id: "trace_tr_scheduled_noop".to_string(),
                audit_sequence_start: 121,
                actor: actor("user_tr_scheduled_noop"),
                workspace_id: None,
            },
        );

        let report = scheduled.run_once().await?;

        assert_eq!(report.attempt.outcome, SchedulerJobOutcome::Noop);
        let sweep = report.sweep.expect("noop still claims and sweeps");
        assert_eq!(sweep.candidate_count, 0);
        assert_eq!(sweep.attempted_count, 0);
        assert!(!sweep.has_more);
        assert!(adapter.called_grant_ids().is_empty());

        let job = scheduler
            .get_job(
                "tenant_tr_scheduled_noop",
                SchedulerJobKind::TokenRefreshSweep,
            )
            .await?
            .expect("scheduled noop job should exist");
        assert_eq!(job.status, SchedulerJobStatus::Pending);
        assert_eq!(job.next_run_at_ms, now_ms + 3_000 + 86_400_000);

        Ok(())
    });
}

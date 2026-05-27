use super::harness::*;

#[test]
fn postgres_live_token_refresh_scheduled_sweep_uses_scheduler_lease_and_reschedules_success() {
    run_live_postgres_test("token_refresh_scheduled_sweep_success", |pool| async move {
        let due_before_ms = 1_748_570_000_000u64;
        let now_ms = 1_748_570_500_000u64;
        let now = UNIX_EPOCH + std::time::Duration::from_millis(now_ms);

        seed_user(
            &pool,
            "tenant_tr_scheduled_success",
            "user_tr_scheduled_success",
        )
        .await?;
        seed_identity(
            &pool,
            "tenant_tr_scheduled_success",
            "identity_tr_scheduled_success",
        )
        .await?;

        let scheduler = PostgresSchedulerJobRepository::new(pool.clone());
        scheduler
            .upsert_job(
                "job_tr_scheduled_success",
                "tenant_tr_scheduled_success",
                SchedulerJobKind::TokenRefreshSweep,
                due_before_ms,
            )
            .await?;

        let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
        let mut due = encrypted_token_grant_record(
            "tenant_tr_scheduled_success",
            "grant_tr_scheduled_due",
            "identity_tr_scheduled_success",
            TokenGrantState::NeedsRefresh,
            "fp-tr-scheduled-old",
        );
        due.expires_at_ms = Some(due_before_ms - 1_000);
        grant_repo.upsert_encrypted_grant(&due).await?;

        let adapter = SequenceRefreshAdapter::new([RefreshOutcome::Success {
            rotated_material: EncryptedGrantMaterial {
                encrypted_primary: vec![0xE1],
                encrypted_renewal: vec![0xF1],
            },
            key_id: "key-tr-scheduled-v2".to_string(),
            new_fingerprint: "fp-tr-scheduled-new".to_string(),
            refreshed_at: now,
            expires_at: Some(UNIX_EPOCH + std::time::Duration::from_millis(1_748_670_000_000)),
        }]);
        let mut ticks = vec![now_ms, now_ms + 2_000];
        ticks.reverse();
        let mut scheduled = PostgresTokenRefreshScheduledSweep::new(
            scheduler.clone(),
            PostgresTokenRefreshSweep::new(pool.clone(), adapter.clone()),
            move || ticks.pop().unwrap_or(now_ms + 9_000),
            TokenRefreshScheduledSweepConfig {
                tenant_id: "tenant_tr_scheduled_success".to_string(),
                lease_id: "lease_tr_scheduled_success".to_string(),
                lease_ms: 10_000,
                retry_delay_ms: 30_000,
                next_run_delay_ms: 86_400_000,
                backlog_next_run_delay_ms: 5_000,
                due_before_ms,
                limit: 4,
                audit_trace_id: "trace_tr_scheduled_success".to_string(),
                audit_sequence_start: 81,
                actor: actor("user_tr_scheduled_success"),
                workspace_id: None,
            },
        );

        let report = scheduled.run_once().await?;

        assert_eq!(report.attempt.outcome, SchedulerJobOutcome::Succeeded);
        assert_eq!(
            report.attempt.lease_id.as_deref(),
            Some("lease_tr_scheduled_success")
        );
        assert_eq!(report.attempt.started_at_ms, now_ms);
        assert_eq!(report.attempt.finished_at_ms, now_ms + 2_000);
        let sweep = report.sweep.expect("scheduled sweep should run");
        assert_eq!(sweep.candidate_count, 1);
        assert_eq!(sweep.attempted_count, 1);
        assert!(!sweep.has_more);
        assert_eq!(
            adapter.called_grant_ids(),
            vec!["grant_tr_scheduled_due".to_string()]
        );

        let job = scheduler
            .get_job(
                "tenant_tr_scheduled_success",
                SchedulerJobKind::TokenRefreshSweep,
            )
            .await?
            .expect("scheduled job should exist");
        assert_eq!(job.status, SchedulerJobStatus::Pending);
        assert_eq!(job.lease_id, None);
        assert_eq!(job.lease_until_ms, None);
        assert_eq!(job.next_run_at_ms, now_ms + 2_000 + 86_400_000);
        assert_eq!(job.last_safe_error_code, None);

        let events = PostgresAuditEventRepository::new(pool.clone())
            .find_by_tenant_and_trace_id(
                "tenant_tr_scheduled_success",
                "trace_tr_scheduled_success",
            )
            .await?;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].sequence, 81);

        let payload: serde_json::Value = sqlx::query_scalar(
            r#"
            SELECT
              jsonb_build_object(
              'before_summary', before_summary,
              'after_summary', after_summary,
              'execution_result', execution_result
            )
            FROM audit_events
            WHERE tenant_id = $1
              AND trace_id = $2
            "#,
        )
        .bind("tenant_tr_scheduled_success")
        .bind("trace_tr_scheduled_success")
        .fetch_one(&pool)
        .await?;
        assert_no_auth_refresh_sensitive_payload(&payload.to_string());
        assert!(!payload.to_string().contains("fp-tr-scheduled"));

        Ok(())
    });
}

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

#[test]
fn postgres_live_token_refresh_scheduled_sweep_reschedules_with_backlog_delay_when_has_more() {
    run_live_postgres_test(
        "token_refresh_scheduled_sweep_backlog_delay",
        |pool| async move {
            let due_before_ms = 1_748_615_000_000u64;
            let now_ms = 1_748_615_500_000u64;
            let now = UNIX_EPOCH + std::time::Duration::from_millis(now_ms);

            seed_user(
                &pool,
                "tenant_tr_scheduled_backlog",
                "user_tr_scheduled_backlog",
            )
            .await?;
            seed_identity(
                &pool,
                "tenant_tr_scheduled_backlog",
                "identity_tr_scheduled_backlog",
            )
            .await?;

            let scheduler = PostgresSchedulerJobRepository::new(pool.clone());
            scheduler
                .upsert_job(
                    "job_tr_scheduled_backlog",
                    "tenant_tr_scheduled_backlog",
                    SchedulerJobKind::TokenRefreshSweep,
                    due_before_ms,
                )
                .await?;

            let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
            let mut due_a = encrypted_token_grant_record(
                "tenant_tr_scheduled_backlog",
                "grant_tr_scheduled_backlog_a",
                "identity_tr_scheduled_backlog",
                TokenGrantState::NeedsRefresh,
                "fp-tr-scheduled-backlog-a-old",
            );
            due_a.expires_at_ms = Some(due_before_ms - 1_000);
            grant_repo.upsert_encrypted_grant(&due_a).await?;

            let mut due_b = encrypted_token_grant_record(
                "tenant_tr_scheduled_backlog",
                "grant_tr_scheduled_backlog_b",
                "identity_tr_scheduled_backlog",
                TokenGrantState::NeedsRefresh,
                "fp-tr-scheduled-backlog-b-old",
            );
            due_b.expires_at_ms = Some(due_before_ms - 2_000);
            grant_repo.upsert_encrypted_grant(&due_b).await?;

            let adapter = SequenceRefreshAdapter::new([RefreshOutcome::Success {
                rotated_material: EncryptedGrantMaterial {
                    encrypted_primary: vec![0xE2],
                    encrypted_renewal: vec![0xF2],
                },
                key_id: "key-tr-scheduled-backlog-v2".to_string(),
                new_fingerprint: "fp-tr-scheduled-backlog-new".to_string(),
                refreshed_at: now,
                expires_at: Some(now + std::time::Duration::from_millis(90_000)),
            }]);
            let mut ticks = vec![now_ms, now_ms + 2_000];
            ticks.reverse();
            let mut scheduled = PostgresTokenRefreshScheduledSweep::new(
                scheduler.clone(),
                PostgresTokenRefreshSweep::new(pool.clone(), adapter.clone()),
                move || ticks.pop().unwrap_or(now_ms + 9_000),
                TokenRefreshScheduledSweepConfig {
                    tenant_id: "tenant_tr_scheduled_backlog".to_string(),
                    lease_id: "lease_tr_scheduled_backlog".to_string(),
                    lease_ms: 10_000,
                    retry_delay_ms: 30_000,
                    next_run_delay_ms: 86_400_000,
                    backlog_next_run_delay_ms: 7_000,
                    due_before_ms,
                    limit: 1,
                    audit_trace_id: "trace_tr_scheduled_backlog".to_string(),
                    audit_sequence_start: 141,
                    actor: actor("user_tr_scheduled_backlog"),
                    workspace_id: None,
                },
            );

            let report = scheduled.run_once().await?;

            assert_eq!(report.attempt.outcome, SchedulerJobOutcome::Succeeded);
            let sweep = report.sweep.expect("scheduled backlog sweep should run");
            assert_eq!(sweep.candidate_count, 1);
            assert_eq!(sweep.attempted_count, 1);
            assert!(sweep.has_more);
            assert_eq!(
                adapter.called_grant_ids(),
                vec!["grant_tr_scheduled_backlog_b".to_string()]
            );

            let job = scheduler
                .get_job(
                    "tenant_tr_scheduled_backlog",
                    SchedulerJobKind::TokenRefreshSweep,
                )
                .await?
                .expect("scheduled backlog job should exist");
            assert_eq!(job.status, SchedulerJobStatus::Pending);
            assert_eq!(job.next_run_at_ms, now_ms + 2_000 + 7_000);
            assert_eq!(job.last_safe_error_code, None);

            Ok(())
        },
    );
}

#[test]
fn postgres_live_token_refresh_scheduled_sweep_reports_lease_lost_after_reclaim() {
    run_live_postgres_test(
        "token_refresh_scheduled_sweep_lease_lost",
        |pool| async move {
            let due_before_ms = 1_748_620_000_000u64;
            let now_ms = 1_748_620_500_000u64;
            let now = UNIX_EPOCH + std::time::Duration::from_millis(now_ms);

            seed_user(
                &pool,
                "tenant_tr_scheduled_lease_lost",
                "user_tr_scheduled_lease_lost",
            )
            .await?;
            seed_identity(
                &pool,
                "tenant_tr_scheduled_lease_lost",
                "identity_tr_scheduled_lease_lost",
            )
            .await?;

            let scheduler = PostgresSchedulerJobRepository::new(pool.clone());
            scheduler
                .upsert_job(
                    "job_tr_scheduled_lease_lost",
                    "tenant_tr_scheduled_lease_lost",
                    SchedulerJobKind::TokenRefreshSweep,
                    due_before_ms,
                )
                .await?;

            let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
            let mut due = encrypted_token_grant_record(
                "tenant_tr_scheduled_lease_lost",
                "grant_tr_scheduled_lease_lost_due",
                "identity_tr_scheduled_lease_lost",
                TokenGrantState::NeedsRefresh,
                "fp-tr-scheduled-lease-lost-old",
            );
            due.expires_at_ms = Some(due_before_ms - 1_000);
            grant_repo.upsert_encrypted_grant(&due).await?;

            let adapter = SequenceRefreshAdapter::new([RefreshOutcome::Success {
                rotated_material: EncryptedGrantMaterial {
                    encrypted_primary: vec![0xA1],
                    encrypted_renewal: vec![0xB1],
                },
                key_id: "key-tr-scheduled-lease-lost-v2".to_string(),
                new_fingerprint: "fp-tr-scheduled-lease-lost-new".to_string(),
                refreshed_at: now,
                expires_at: Some(now + std::time::Duration::from_millis(90_000)),
            }]);

            let scheduler_for_clock = scheduler.clone();
            let mut tick_count = 0u8;
            let mut scheduled = PostgresTokenRefreshScheduledSweep::new(
                scheduler.clone(),
                PostgresTokenRefreshSweep::new(pool.clone(), adapter.clone()),
                move || {
                    tick_count = tick_count.saturating_add(1);
                    if tick_count == 2 {
                        runtime().block_on(async {
                            let reclaimed = scheduler_for_clock
                                .try_acquire(
                                    "tenant_tr_scheduled_lease_lost",
                                    SchedulerJobKind::TokenRefreshSweep,
                                    now_ms + 11_000,
                                    "lease_tr_scheduled_lease_lost_new_owner",
                                    now_ms + 30_000,
                                )
                                .await
                                .expect("stale lease reclaim should execute");
                            assert!(
                            matches!(reclaimed, SchedulerLeaseAcquire::Acquired(_)),
                            "expected second worker to reclaim expired lease, got {reclaimed:?}"
                        );
                        });
                    }
                    now_ms + u64::from(tick_count) * 1_000
                },
                TokenRefreshScheduledSweepConfig {
                    tenant_id: "tenant_tr_scheduled_lease_lost".to_string(),
                    lease_id: "lease_tr_scheduled_lease_lost_old_owner".to_string(),
                    lease_ms: 10_000,
                    retry_delay_ms: 30_000,
                    next_run_delay_ms: 86_400_000,
                    backlog_next_run_delay_ms: 5_000,
                    due_before_ms,
                    limit: 4,
                    audit_trace_id: "trace_tr_scheduled_lease_lost".to_string(),
                    audit_sequence_start: 131,
                    actor: actor("user_tr_scheduled_lease_lost"),
                    workspace_id: None,
                },
            );

            let report = scheduled.run_once().await?;

            assert_eq!(report.attempt.outcome, SchedulerJobOutcome::LeaseLost);
            assert_eq!(
                report.attempt.lease_id.as_deref(),
                Some("lease_tr_scheduled_lease_lost_old_owner")
            );
            let sweep = report
                .sweep
                .expect("sweep should have executed before finalize");
            assert_eq!(sweep.candidate_count, 1);
            assert_eq!(sweep.attempted_count, 1);
            assert!(!sweep.has_more);
            assert_eq!(
                adapter.called_grant_ids(),
                vec!["grant_tr_scheduled_lease_lost_due".to_string()]
            );

            let job = scheduler
                .get_job(
                    "tenant_tr_scheduled_lease_lost",
                    SchedulerJobKind::TokenRefreshSweep,
                )
                .await?
                .expect("scheduled lease-lost job should exist");
            assert_eq!(
                job.lease_id.as_deref(),
                Some("lease_tr_scheduled_lease_lost_new_owner")
            );
            assert_eq!(job.status, SchedulerJobStatus::Running);

            Ok(())
        },
    );
}

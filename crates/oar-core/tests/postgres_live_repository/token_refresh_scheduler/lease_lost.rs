use super::*;

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

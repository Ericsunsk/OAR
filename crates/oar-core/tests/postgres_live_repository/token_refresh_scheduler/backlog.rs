use super::*;

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

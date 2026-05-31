use super::*;

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
        assert_eq!(job.id, TOKEN_REFRESH_SWEEP_SCHEDULER_JOB_ID);
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

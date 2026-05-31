use super::*;

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

        let job = PostgresSchedulerJobRepository::new(pool.clone())
            .get_job(
                "tenant_maintenance_success",
                SchedulerJobKind::TokenRefreshSweep,
            )
            .await?
            .expect("scheduled job should be bootstrapped by maintenance tick");
        assert_eq!(job.id, TOKEN_REFRESH_SWEEP_SCHEDULER_JOB_ID);
        assert_eq!(job.status, SchedulerJobStatus::Pending);

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

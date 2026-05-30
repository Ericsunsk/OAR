use super::*;

#[test]
fn postgres_live_token_refresh_sweep_reports_backlog_when_due_exceeds_limit() {
    run_live_postgres_test("token_refresh_sweep_backlog_has_more", |pool| async move {
        let due_before_ms = 1_748_565_000_000u64;
        let due_before = UNIX_EPOCH + std::time::Duration::from_millis(due_before_ms);
        let now = UNIX_EPOCH + std::time::Duration::from_millis(1_748_565_500_000);

        seed_user(&pool, "tenant_tr_sweep_backlog", "user_tr_sweep_backlog").await?;
        seed_identity(
            &pool,
            "tenant_tr_sweep_backlog",
            "identity_tr_sweep_backlog",
        )
        .await?;

        let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
        let mut due_a = encrypted_token_grant_record(
            "tenant_tr_sweep_backlog",
            "grant_sweep_backlog_a",
            "identity_tr_sweep_backlog",
            TokenGrantState::NeedsRefresh,
            "fp-sweep-backlog-a-old",
        );
        due_a.expires_at_ms = Some(due_before_ms - 1_000);
        grant_repo.upsert_encrypted_grant(&due_a).await?;

        let mut due_b = encrypted_token_grant_record(
            "tenant_tr_sweep_backlog",
            "grant_sweep_backlog_b",
            "identity_tr_sweep_backlog",
            TokenGrantState::NeedsRefresh,
            "fp-sweep-backlog-b-old",
        );
        due_b.expires_at_ms = Some(due_before_ms - 2_000);
        grant_repo.upsert_encrypted_grant(&due_b).await?;

        let adapter = SequenceRefreshAdapter::new([RefreshOutcome::Success {
            rotated_material: EncryptedGrantMaterial {
                encrypted_primary: vec![0xC2],
                encrypted_renewal: vec![0xD2],
            },
            key_id: "key-sweep-backlog-v2".to_string(),
            new_fingerprint: "fp-sweep-backlog-new".to_string(),
            refreshed_at: now,
            expires_at: None,
        }]);
        let mut sweep = PostgresTokenRefreshSweep::new(pool.clone(), adapter.clone());

        let report = sweep
            .run_once_for_tenant(PostgresTokenRefreshSweepRequest {
                tenant_id: "tenant_tr_sweep_backlog".to_string(),
                due_before,
                limit: 1,
                now,
                audit_trace_id: "trace_token_refresh_sweep_backlog".to_string(),
                audit_sequence_start: 161,
                occurred_at_ms: 1_748_565_500_111,
                actor: actor("user_tr_sweep_backlog"),
                workspace_id: None,
            })
            .await?;

        assert_eq!(report.candidate_count, 1);
        assert_eq!(report.attempted_count, 1);
        assert!(report.has_more);
        assert_eq!(report.reports.len(), 1);
        assert_eq!(
            adapter.called_grant_ids(),
            vec!["grant_sweep_backlog_b".to_string()]
        );

        let first_stored = grant_repo
            .get_by_id("tenant_tr_sweep_backlog", "grant_sweep_backlog_b")
            .await?
            .expect("first backlog grant should exist");
        assert_eq!(first_stored.state, TokenGrantState::Valid);
        assert_eq!(first_stored.oauth_grant_fingerprint, "fp-sweep-backlog-new");

        let second_stored = grant_repo
            .get_by_id("tenant_tr_sweep_backlog", "grant_sweep_backlog_a")
            .await?
            .expect("second backlog grant should remain");
        assert_eq!(second_stored.state, TokenGrantState::NeedsRefresh);
        assert_eq!(
            second_stored.oauth_grant_fingerprint,
            "fp-sweep-backlog-a-old"
        );

        let events = PostgresAuditEventRepository::new(pool.clone())
            .find_by_tenant_and_trace_id(
                "tenant_tr_sweep_backlog",
                "trace_token_refresh_sweep_backlog",
            )
            .await?;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].sequence, 161);

        Ok(())
    });
}

use super::*;

#[test]
fn postgres_live_token_refresh_sweep_limit_zero_short_circuits_without_adapter_or_audit() {
    run_live_postgres_test("token_refresh_sweep_limit_zero", |pool| async move {
        let due_before_ms = 1_748_560_000_000u64;
        let due_before = UNIX_EPOCH + std::time::Duration::from_millis(due_before_ms);
        let now = UNIX_EPOCH + std::time::Duration::from_millis(1_748_560_500_000);

        seed_user(&pool, "tenant_tr_sweep_zero", "user_tr_sweep_zero").await?;
        seed_identity(&pool, "tenant_tr_sweep_zero", "identity_tr_sweep_zero").await?;

        let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
        let mut due = encrypted_token_grant_record(
            "tenant_tr_sweep_zero",
            "grant_sweep_zero_due",
            "identity_tr_sweep_zero",
            TokenGrantState::NeedsRefresh,
            "fp-sweep-zero-old",
        );
        due.expires_at_ms = Some(due_before_ms - 1_000);
        grant_repo.upsert_encrypted_grant(&due).await?;

        let adapter = SequenceRefreshAdapter::new([RefreshOutcome::Success {
            rotated_material: EncryptedGrantMaterial {
                encrypted_primary: vec![0xC1],
                encrypted_renewal: vec![0xD1],
            },
            key_id: "key-sweep-zero-unused".to_string(),
            new_fingerprint: "fp-sweep-zero-new".to_string(),
            refreshed_at: now,
            expires_at: None,
        }]);
        let mut sweep = PostgresTokenRefreshSweep::new(pool.clone(), adapter.clone());

        let report = sweep
            .run_once_for_tenant(PostgresTokenRefreshSweepRequest {
                tenant_id: "tenant_tr_sweep_zero".to_string(),
                due_before,
                limit: 0,
                now,
                audit_trace_id: "trace_token_refresh_sweep_zero".to_string(),
                audit_sequence_start: 71,
                occurred_at_ms: 1_748_560_500_111,
                actor: actor("user_tr_sweep_zero"),
                workspace_id: None,
            })
            .await?;

        assert_eq!(report.candidate_count, 0);
        assert_eq!(report.attempted_count, 0);
        assert!(!report.has_more);
        assert!(report.reports.is_empty());
        assert!(adapter.called_grant_ids().is_empty());

        let stored = grant_repo
            .get_by_id("tenant_tr_sweep_zero", "grant_sweep_zero_due")
            .await?
            .expect("limit zero grant should remain");
        assert_eq!(stored.state, TokenGrantState::NeedsRefresh);
        assert_eq!(stored.oauth_grant_fingerprint, "fp-sweep-zero-old");
        assert_eq!(stored.oauth_grant_key_id, "key-v1");

        let events = PostgresAuditEventRepository::new(pool.clone())
            .find_by_tenant_and_trace_id("tenant_tr_sweep_zero", "trace_token_refresh_sweep_zero")
            .await?;
        assert!(events.is_empty());

        Ok(())
    });
}

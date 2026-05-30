use super::*;

#[test]
fn postgres_live_token_refresh_candidate_selection_scopes_filters_and_orders() {
    run_live_postgres_test("token_refresh_candidate_selection", |pool| async move {
        let due_before_ms = 1_748_300_000_000u64;
        let due_before = UNIX_EPOCH + std::time::Duration::from_millis(due_before_ms);

        seed_user(&pool, "tenant_tg_candidates", "user_tg_candidates").await?;
        seed_identity(&pool, "tenant_tg_candidates", "identity_tg_candidates").await?;
        seed_user(
            &pool,
            "tenant_tg_candidates_other",
            "user_tg_candidates_other",
        )
        .await?;
        seed_identity(
            &pool,
            "tenant_tg_candidates_other",
            "identity_tg_candidates_other",
        )
        .await?;

        let repository = PostgresTokenGrantRepository::new(pool.clone());

        let mut due_valid = encrypted_token_grant_record(
            "tenant_tg_candidates",
            "grant_due_valid",
            "identity_tg_candidates",
            TokenGrantState::Valid,
            "fp-due-valid",
        );
        due_valid.expires_at_ms = Some(due_before_ms - 1_000);
        due_valid.last_refresh_error = None;
        repository.upsert_encrypted_grant(&due_valid).await?;

        let mut due_needs_refresh = encrypted_token_grant_record(
            "tenant_tg_candidates",
            "grant_due_needs_refresh",
            "identity_tg_candidates",
            TokenGrantState::NeedsRefresh,
            "fp-due-needs",
        );
        due_needs_refresh.expires_at_ms = Some(due_before_ms + 500_000);
        repository
            .upsert_encrypted_grant(&due_needs_refresh)
            .await?;

        let mut due_expired = encrypted_token_grant_record(
            "tenant_tg_candidates",
            "grant_due_expired",
            "identity_tg_candidates",
            TokenGrantState::Expired,
            "fp-due-expired",
        );
        due_expired.expires_at_ms = Some(due_before_ms + 100_000);
        repository.upsert_encrypted_grant(&due_expired).await?;

        let mut future_valid = encrypted_token_grant_record(
            "tenant_tg_candidates",
            "grant_future_valid",
            "identity_tg_candidates",
            TokenGrantState::Valid,
            "fp-future-valid",
        );
        future_valid.expires_at_ms = Some(due_before_ms + 86_400_000);
        repository.upsert_encrypted_grant(&future_valid).await?;

        let revoked = encrypted_token_grant_record(
            "tenant_tg_candidates",
            "grant_revoked",
            "identity_tg_candidates",
            TokenGrantState::Valid,
            "fp-revoked-candidate",
        );
        repository.upsert_encrypted_grant(&revoked).await?;
        repository
            .revoke(
                "tenant_tg_candidates",
                "grant_revoked",
                due_before_ms - 500,
                "manual revoke",
            )
            .await?
            .expect("revoke should update row");

        let reauth_required = encrypted_token_grant_record(
            "tenant_tg_candidates",
            "grant_reauth_required",
            "identity_tg_candidates",
            TokenGrantState::Valid,
            "fp-reauth-candidate",
        );
        repository.upsert_encrypted_grant(&reauth_required).await?;
        repository
            .mark_reauth_required(
                "tenant_tg_candidates",
                "grant_reauth_required",
                "fp-reauth-candidate",
                due_before_ms - 250,
                "invalid_grant",
            )
            .await?
            .expect("mark reauth required should update row");

        let mut empty_encrypted = encrypted_token_grant_record(
            "tenant_tg_candidates",
            "grant_empty_encrypted",
            "identity_tg_candidates",
            TokenGrantState::NeedsRefresh,
            "fp-empty-encrypted",
        );
        empty_encrypted.encrypted_oauth_grant = Vec::new();
        repository.upsert_encrypted_grant(&empty_encrypted).await?;

        let mut other_tenant_due = encrypted_token_grant_record(
            "tenant_tg_candidates_other",
            "grant_other_tenant_due",
            "identity_tg_candidates_other",
            TokenGrantState::NeedsRefresh,
            "fp-other-tenant",
        );
        other_tenant_due.expires_at_ms = Some(due_before_ms - 2_000);
        repository.upsert_encrypted_grant(&other_tenant_due).await?;

        let candidates = repository
            .list_refresh_candidate_snapshots("tenant_tg_candidates", due_before, 32)
            .await?;

        let ids: Vec<&str> = candidates
            .iter()
            .map(|candidate| candidate.grant_id.0.as_str())
            .collect();
        assert_eq!(
            ids,
            vec![
                "grant_due_expired",
                "grant_due_needs_refresh",
                "grant_due_valid"
            ]
        );

        for snapshot in &candidates {
            assert_eq!(snapshot.tenant_id.0, "tenant_tg_candidates");
            assert!(snapshot.expected_fingerprint.starts_with("fp-"));
            assert!(snapshot.has_refresh_material);
            assert_eq!(snapshot.revoked_at, None);
            assert_eq!(snapshot.reauth_required_at, None);
        }
        assert_eq!(candidates[0].state, TokenGrantState::Expired);
        assert_eq!(candidates[1].state, TokenGrantState::NeedsRefresh);
        assert_eq!(candidates[2].state, TokenGrantState::Valid);

        let limited = repository
            .list_refresh_candidate_snapshots("tenant_tg_candidates", due_before, 2)
            .await?;
        let limited_ids: Vec<&str> = limited
            .iter()
            .map(|candidate| candidate.grant_id.0.as_str())
            .collect();
        assert_eq!(
            limited_ids,
            vec!["grant_due_expired", "grant_due_needs_refresh"]
        );

        let none = repository
            .list_refresh_candidate_snapshots("tenant_tg_candidates", due_before, 0)
            .await?;
        assert!(none.is_empty());

        Ok(())
    });
}

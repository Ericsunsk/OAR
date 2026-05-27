use super::*;

#[test]
fn postgres_live_token_grant_rotate_cas_succeeds_and_updates_fields() {
    run_live_postgres_test("token_grant_rotate_success", |pool| async move {
        seed_user(&pool, "tenant_tg_rotate_ok", "user_tg_rotate_ok").await?;
        seed_identity(&pool, "tenant_tg_rotate_ok", "identity_tg_rotate_ok").await?;

        let repository = PostgresTokenGrantRepository::new(pool.clone());
        let initial = encrypted_token_grant_record(
            "tenant_tg_rotate_ok",
            "grant_tg_rotate_ok",
            "identity_tg_rotate_ok",
            TokenGrantState::NeedsRefresh,
            "fp-old",
        );
        repository.upsert_encrypted_grant(&initial).await?;

        let rotated = repository
            .rotate_encrypted_grant(rotate_grant_request(
                "tenant_tg_rotate_ok",
                "grant_tg_rotate_ok",
                "fp-old",
                &[0xAA, 0xBB, 0xCC],
            ))
            .await?
            .expect("rotation should return updated row");

        assert_eq!(rotated.state, TokenGrantState::Valid);
        assert_eq!(rotated.oauth_grant_fingerprint, "fp-new");
        assert_eq!(rotated.oauth_grant_key_id, "key-v2");
        assert_eq!(rotated.encrypted_oauth_grant, vec![0xAA, 0xBB, 0xCC]);
        assert_eq!(rotated.expires_at_ms, Some(1_748_270_000_000));
        assert_eq!(rotated.refreshed_at_ms, Some(1_748_260_500_000));
        assert_eq!(rotated.last_refresh_error, None);
        assert_eq!(rotated.revoked_at_ms, None);
        assert_eq!(rotated.reauth_required_at_ms, None);

        Ok(())
    });
}

#[test]
fn postgres_live_token_grant_rotate_with_stale_fingerprint_is_noop() {
    run_live_postgres_test("token_grant_rotate_stale_fp", |pool| async move {
        seed_user(&pool, "tenant_tg_rotate_stale", "user_tg_rotate_stale").await?;
        seed_identity(&pool, "tenant_tg_rotate_stale", "identity_tg_rotate_stale").await?;

        let repository = PostgresTokenGrantRepository::new(pool.clone());
        let initial = encrypted_token_grant_record(
            "tenant_tg_rotate_stale",
            "grant_tg_rotate_stale",
            "identity_tg_rotate_stale",
            TokenGrantState::Valid,
            "fp-current",
        );
        repository.upsert_encrypted_grant(&initial).await?;

        let rotated = repository
            .rotate_encrypted_grant(rotate_grant_request(
                "tenant_tg_rotate_stale",
                "grant_tg_rotate_stale",
                "fp-stale",
                &[0xAA],
            ))
            .await?;
        assert_eq!(rotated, None);

        Ok(())
    });
}

#[test]
fn postgres_live_token_grant_rotate_blocked_after_revoke() {
    run_live_postgres_test("token_grant_rotate_blocked_revoked", |pool| async move {
        seed_user(&pool, "tenant_tg_rotate_revoked", "user_tg_rotate_revoked").await?;
        seed_identity(
            &pool,
            "tenant_tg_rotate_revoked",
            "identity_tg_rotate_revoked",
        )
        .await?;

        let repository = PostgresTokenGrantRepository::new(pool.clone());
        let initial = encrypted_token_grant_record(
            "tenant_tg_rotate_revoked",
            "grant_tg_rotate_revoked",
            "identity_tg_rotate_revoked",
            TokenGrantState::Valid,
            "fp-revoked",
        );
        repository.upsert_encrypted_grant(&initial).await?;
        repository
            .revoke(
                "tenant_tg_rotate_revoked",
                "grant_tg_rotate_revoked",
                1_748_260_000_000,
                "user disconnected",
            )
            .await?
            .expect("revoke should update row");

        let rotated = repository
            .rotate_encrypted_grant(rotate_grant_request(
                "tenant_tg_rotate_revoked",
                "grant_tg_rotate_revoked",
                "fp-revoked",
                &[0xAA],
            ))
            .await?;
        assert_eq!(rotated, None);

        Ok(())
    });
}

#[test]
fn postgres_live_token_grant_rotate_blocked_after_reauth_required() {
    run_live_postgres_test("token_grant_rotate_blocked_reauth", |pool| async move {
        seed_user(&pool, "tenant_tg_rotate_reauth", "user_tg_rotate_reauth").await?;
        seed_identity(
            &pool,
            "tenant_tg_rotate_reauth",
            "identity_tg_rotate_reauth",
        )
        .await?;

        let repository = PostgresTokenGrantRepository::new(pool.clone());
        let initial = encrypted_token_grant_record(
            "tenant_tg_rotate_reauth",
            "grant_tg_rotate_reauth",
            "identity_tg_rotate_reauth",
            TokenGrantState::Valid,
            "fp-reauth",
        );
        repository.upsert_encrypted_grant(&initial).await?;
        repository
            .mark_reauth_required(
                "tenant_tg_rotate_reauth",
                "grant_tg_rotate_reauth",
                "fp-reauth",
                1_748_260_000_000,
                "invalid_grant",
            )
            .await?
            .expect("mark reauth required should update row");

        let rotated = repository
            .rotate_encrypted_grant(rotate_grant_request(
                "tenant_tg_rotate_reauth",
                "grant_tg_rotate_reauth",
                "fp-reauth",
                &[0xAA],
            ))
            .await?;
        assert_eq!(rotated, None);

        Ok(())
    });
}

#[test]
fn postgres_live_token_grant_mark_refresh_failed_sets_needs_refresh_and_error() {
    run_live_postgres_test("token_grant_refresh_failed", |pool| async move {
        seed_user(&pool, "tenant_tg_refresh_failed", "user_tg_refresh_failed").await?;
        seed_identity(
            &pool,
            "tenant_tg_refresh_failed",
            "identity_tg_refresh_failed",
        )
        .await?;

        let repository = PostgresTokenGrantRepository::new(pool.clone());
        let initial = encrypted_token_grant_record(
            "tenant_tg_refresh_failed",
            "grant_tg_refresh_failed",
            "identity_tg_refresh_failed",
            TokenGrantState::Valid,
            "fp-refresh-fail",
        );
        repository.upsert_encrypted_grant(&initial).await?;

        let updated = repository
            .mark_refresh_failed(
                "tenant_tg_refresh_failed",
                "grant_tg_refresh_failed",
                "fp-refresh-fail",
                1_748_260_010_000,
                "network timeout",
            )
            .await?
            .expect("refresh failure should return updated row");

        assert_eq!(updated.state, TokenGrantState::NeedsRefresh);
        assert_eq!(
            updated.last_refresh_error.as_deref(),
            Some("network timeout")
        );
        assert_eq!(updated.refreshed_at_ms, Some(1_748_260_010_000));
        assert_eq!(updated.oauth_grant_fingerprint, "fp-refresh-fail");

        Ok(())
    });
}

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

#[test]
fn postgres_live_token_grant_apply_refresh_command_dispatches_rotate() {
    run_live_postgres_test("token_grant_apply_refresh_rotate", |pool| async move {
        seed_user(
            &pool,
            "tenant_tg_apply_refresh_rotate",
            "user_tg_apply_refresh_rotate",
        )
        .await?;
        seed_identity(
            &pool,
            "tenant_tg_apply_refresh_rotate",
            "identity_tg_apply_refresh_rotate",
        )
        .await?;

        let repository = PostgresTokenGrantRepository::new(pool.clone());
        let initial = encrypted_token_grant_record(
            "tenant_tg_apply_refresh_rotate",
            "grant_tg_apply_refresh_rotate",
            "identity_tg_apply_refresh_rotate",
            TokenGrantState::NeedsRefresh,
            "fp-apply-old",
        );
        repository.upsert_encrypted_grant(&initial).await?;

        let updated = repository
            .apply_refresh_command(TokenRefreshRepositoryCommand::RotateGrantCas {
                grant_id: TokenGrantId("grant_tg_apply_refresh_rotate".to_string()),
                tenant_id: TenantId("tenant_tg_apply_refresh_rotate".to_string()),
                expected_fingerprint: "fp-apply-old".to_string(),
                expires_at_ms: Some(1_748_280_000_000),
                refreshed_at_ms: 1_748_270_500_000,
                encrypted_grant_blob: EncryptedGrantBlob(vec![0xD0, 0xD1, 0xD2]),
                grant_key_id: "key-v2".to_string(),
                new_fingerprint: "fp-apply-new".to_string(),
            })
            .await?
            .expect("apply refresh rotate should return updated row");

        assert_eq!(updated.state, TokenGrantState::Valid);
        assert_eq!(updated.oauth_grant_fingerprint, "fp-apply-new");
        assert_eq!(updated.oauth_grant_key_id, "key-v2");
        assert_eq!(updated.encrypted_oauth_grant, vec![0xD0, 0xD1, 0xD2]);
        assert_eq!(updated.expires_at_ms, Some(1_748_280_000_000));
        assert_eq!(updated.refreshed_at_ms, Some(1_748_270_500_000));
        assert_eq!(updated.last_refresh_error, None);

        Ok(())
    });
}

#[test]
fn postgres_live_token_grant_apply_refresh_command_dispatches_mark_needs_refresh() {
    run_live_postgres_test(
        "token_grant_apply_refresh_needs_refresh",
        |pool| async move {
            seed_user(
                &pool,
                "tenant_tg_apply_refresh_needs",
                "user_tg_apply_refresh_needs",
            )
            .await?;
            seed_identity(
                &pool,
                "tenant_tg_apply_refresh_needs",
                "identity_tg_apply_refresh_needs",
            )
            .await?;

            let repository = PostgresTokenGrantRepository::new(pool.clone());
            let initial = encrypted_token_grant_record(
                "tenant_tg_apply_refresh_needs",
                "grant_tg_apply_refresh_needs",
                "identity_tg_apply_refresh_needs",
                TokenGrantState::Valid,
                "fp-apply-needs",
            );
            repository.upsert_encrypted_grant(&initial).await?;

            let updated = repository
                .apply_refresh_command(TokenRefreshRepositoryCommand::MarkNeedsRefresh {
                    grant_id: TokenGrantId("grant_tg_apply_refresh_needs".to_string()),
                    tenant_id: TenantId("tenant_tg_apply_refresh_needs".to_string()),
                    expected_fingerprint: "fp-apply-needs".to_string(),
                    refreshed_at_ms: 1_748_270_700_000,
                    safe_error: "refresh_token=rt_fake Authorization: Bearer at_fake".to_string(),
                })
                .await?
                .expect("apply refresh mark needs refresh should return updated row");

            assert_eq!(updated.state, TokenGrantState::NeedsRefresh);
            assert_eq!(
                updated.last_refresh_error.as_deref(),
                Some("<redacted refresh error>")
            );
            assert_eq!(updated.refreshed_at_ms, Some(1_748_270_700_000));
            assert_eq!(updated.oauth_grant_fingerprint, "fp-apply-needs");

            Ok(())
        },
    );
}

#[test]
fn postgres_live_token_grant_apply_refresh_command_dispatches_mark_reauth_required() {
    run_live_postgres_test(
        "token_grant_apply_refresh_reauth_required",
        |pool| async move {
            seed_user(
                &pool,
                "tenant_tg_apply_refresh_reauth",
                "user_tg_apply_refresh_reauth",
            )
            .await?;
            seed_identity(
                &pool,
                "tenant_tg_apply_refresh_reauth",
                "identity_tg_apply_refresh_reauth",
            )
            .await?;

            let repository = PostgresTokenGrantRepository::new(pool.clone());
            let initial = encrypted_token_grant_record(
                "tenant_tg_apply_refresh_reauth",
                "grant_tg_apply_refresh_reauth",
                "identity_tg_apply_refresh_reauth",
                TokenGrantState::Valid,
                "fp-apply-reauth",
            );
            repository.upsert_encrypted_grant(&initial).await?;

            let updated = repository
                .apply_refresh_command(TokenRefreshRepositoryCommand::MarkReauthRequired {
                    grant_id: TokenGrantId("grant_tg_apply_refresh_reauth".to_string()),
                    tenant_id: TenantId("tenant_tg_apply_refresh_reauth".to_string()),
                    expected_fingerprint: "fp-apply-reauth".to_string(),
                    reauth_required_at_ms: 1_748_270_900_000,
                    safe_error: "invalid_grant".to_string(),
                })
                .await?
                .expect("apply refresh mark reauth required should return updated row");

            assert_eq!(updated.state, TokenGrantState::ReauthRequired);
            assert_eq!(updated.last_refresh_error.as_deref(), Some("invalid_grant"));
            assert_eq!(updated.reauth_required_at_ms, Some(1_748_270_900_000));
            assert_eq!(updated.oauth_grant_fingerprint, "fp-apply-reauth");

            Ok(())
        },
    );
}

#[test]
fn postgres_live_token_grant_apply_refresh_command_dispatches_mark_config_required() {
    run_live_postgres_test(
        "token_grant_apply_refresh_config_required",
        |pool| async move {
            seed_user(
                &pool,
                "tenant_tg_apply_refresh_config",
                "user_tg_apply_refresh_config",
            )
            .await?;
            seed_identity(
                &pool,
                "tenant_tg_apply_refresh_config",
                "identity_tg_apply_refresh_config",
            )
            .await?;

            let repository = PostgresTokenGrantRepository::new(pool.clone());
            let initial = encrypted_token_grant_record(
                "tenant_tg_apply_refresh_config",
                "grant_tg_apply_refresh_config",
                "identity_tg_apply_refresh_config",
                TokenGrantState::Valid,
                "fp-apply-config",
            );
            repository.upsert_encrypted_grant(&initial).await?;

            let updated = repository
                .apply_refresh_command(TokenRefreshRepositoryCommand::MarkConfigRequired {
                    grant_id: TokenGrantId("grant_tg_apply_refresh_config".to_string()),
                    tenant_id: TenantId("tenant_tg_apply_refresh_config".to_string()),
                    expected_fingerprint: "fp-apply-config".to_string(),
                    refreshed_at_ms: 1_748_271_000_000,
                    safe_error: "refresh_config_required".to_string(),
                })
                .await?
                .expect("apply refresh mark config required should return updated row");

            assert_eq!(updated.state, TokenGrantState::NeedsRefresh);
            assert_eq!(
                updated.last_refresh_error.as_deref(),
                Some("refresh_config_required")
            );
            assert_eq!(updated.refreshed_at_ms, Some(1_748_271_000_000));

            let candidates = repository
                .list_refresh_candidate_snapshots(
                    "tenant_tg_apply_refresh_config",
                    UNIX_EPOCH + std::time::Duration::from_millis(1_748_272_000_000),
                    8,
                )
                .await?;
            assert!(candidates.is_empty());

            Ok(())
        },
    );
}

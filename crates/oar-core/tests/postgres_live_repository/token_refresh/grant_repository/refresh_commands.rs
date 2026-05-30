use super::*;

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

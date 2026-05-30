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

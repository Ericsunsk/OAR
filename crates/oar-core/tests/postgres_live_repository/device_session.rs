use super::harness::*;

#[test]
fn postgres_live_device_session_upsert_lookup_and_tenant_scope() {
    run_live_postgres_test("device_session_upsert_lookup_scope", |pool| async move {
        seed_user(&pool, "tenant_ds_a", "user_ds_a").await?;
        seed_user(&pool, "tenant_ds_b", "user_ds_b").await?;

        let repository = PostgresDeviceSessionRepository::new(pool.clone());
        let now = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_748_270_000_000);
        let session = device_session(
            "tenant_ds_a",
            "user_ds_a",
            "session_ds_01",
            "okr_evidence",
            7,
            now,
        );

        let stored = repository
            .upsert_with_identity_hash(&session, "sha256:session-ds-01")
            .await?;
        assert_eq!(stored.tenant_id, "tenant_ds_a");
        assert_eq!(stored.id, "session_ds_01");
        assert_eq!(stored.sync_cursor_value, 7);
        assert_eq!(stored.session_identity_hash, "sha256:session-ds-01");
        assert_eq!(stored.state, SessionState::Active);

        let found = repository
            .get_by_id("tenant_ds_a", "session_ds_01")
            .await?
            .expect("session should be found in tenant A");
        assert_eq!(found.id, "session_ds_01");
        assert_eq!(found.tenant_id, "tenant_ds_a");

        let hidden_from_other_tenant = repository.get_by_id("tenant_ds_b", "session_ds_01").await?;
        assert_eq!(hidden_from_other_tenant, None);

        let conflicting_tenant_session = device_session(
            "tenant_ds_b",
            "user_ds_b",
            "session_ds_01",
            "okr_evidence",
            8,
            now + std::time::Duration::from_secs(10),
        );
        let cross_tenant_result = repository
            .upsert_with_identity_hash(&conflicting_tenant_session, "sha256:session-ds-02")
            .await;
        match cross_tenant_result {
            Err(PostgresRepositoryError::TenantMismatch {
                field,
                expected,
                actual,
            }) => {
                assert_eq!(field, "tenant_id");
                assert_eq!(expected, "tenant_ds_b");
                assert_eq!(actual, "<redacted>");
            }
            other => panic!("expected tenant mismatch, got {other:?}"),
        }

        Ok(())
    });
}

#[test]
fn postgres_live_device_session_advance_cursor_cas_and_terminal_state_guards() {
    run_live_postgres_test("device_session_advance_cursor_cas", |pool| async move {
        seed_user(&pool, "tenant_ds_cas", "user_ds_cas").await?;

        let repository = PostgresDeviceSessionRepository::new(pool.clone());
        let base = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_748_280_000_000);
        let session = device_session(
            "tenant_ds_cas",
            "user_ds_cas",
            "session_ds_cas",
            "okr_evidence",
            10,
            base,
        );
        repository
            .upsert_with_identity_hash(&session, "sha256:session-ds-cas")
            .await?;

        let advanced = repository
            .advance_cursor_cas(
                "tenant_ds_cas",
                "session_ds_cas",
                10,
                11,
                base + std::time::Duration::from_secs(10),
            )
            .await?;
        assert_eq!(
            advanced.expect("advance should succeed").sync_cursor_value,
            11
        );

        let backwards_now = repository
            .advance_cursor_cas(
                "tenant_ds_cas",
                "session_ds_cas",
                11,
                12,
                base + std::time::Duration::from_secs(5),
            )
            .await?;
        assert_eq!(backwards_now, None);

        let after_backwards_attempt = repository
            .get_by_id("tenant_ds_cas", "session_ds_cas")
            .await?
            .expect("session should still exist");
        assert_eq!(after_backwards_attempt.sync_cursor_value, 11);
        assert_eq!(
            after_backwards_attempt.sync_cursor_updated_at,
            base + std::time::Duration::from_secs(10)
        );
        assert_eq!(
            after_backwards_attempt.last_seen_at,
            base + std::time::Duration::from_secs(10)
        );

        let stale = repository
            .advance_cursor_cas(
                "tenant_ds_cas",
                "session_ds_cas",
                10,
                12,
                base + std::time::Duration::from_secs(20),
            )
            .await?;
        assert_eq!(stale, None);

        let non_monotonic = repository
            .advance_cursor_cas(
                "tenant_ds_cas",
                "session_ds_cas",
                11,
                11,
                base + std::time::Duration::from_secs(25),
            )
            .await?;
        assert_eq!(non_monotonic, None);

        let revoked = repository
            .revoke(
                "tenant_ds_cas",
                "session_ds_cas",
                base + std::time::Duration::from_secs(30),
            )
            .await?;
        assert_eq!(
            revoked.expect("revoke should apply").state,
            SessionState::Revoked
        );

        let blocked_after_revoke = repository
            .advance_cursor_cas(
                "tenant_ds_cas",
                "session_ds_cas",
                11,
                12,
                base + std::time::Duration::from_secs(40),
            )
            .await?;
        assert_eq!(blocked_after_revoke, None);

        Ok(())
    });
}

#[test]
fn postgres_live_device_session_expire_blocks_cursor_and_revoke_is_idempotent() {
    run_live_postgres_test("device_session_expire_and_revoke", |pool| async move {
        seed_user(&pool, "tenant_ds_exp", "user_ds_exp").await?;

        let repository = PostgresDeviceSessionRepository::new(pool.clone());
        let base = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_748_290_000_000);
        let session = device_session(
            "tenant_ds_exp",
            "user_ds_exp",
            "session_ds_exp",
            "okr_evidence",
            20,
            base,
        );
        repository
            .upsert_with_identity_hash(&session, "sha256:session-ds-exp")
            .await?;

        let expired = repository
            .expire(
                "tenant_ds_exp",
                "session_ds_exp",
                base + std::time::Duration::from_secs(10),
            )
            .await?;
        assert_eq!(
            expired.expect("expire should apply").state,
            SessionState::Expired
        );

        let blocked_after_expire = repository
            .advance_cursor_cas(
                "tenant_ds_exp",
                "session_ds_exp",
                20,
                21,
                base + std::time::Duration::from_secs(20),
            )
            .await?;
        assert_eq!(blocked_after_expire, None);

        let first_revoke = repository
            .revoke(
                "tenant_ds_exp",
                "session_ds_exp",
                base + std::time::Duration::from_secs(30),
            )
            .await?;
        assert_eq!(
            first_revoke
                .expect("revoke after expire should apply")
                .state,
            SessionState::Revoked
        );

        let second_revoke = repository
            .revoke(
                "tenant_ds_exp",
                "session_ds_exp",
                base + std::time::Duration::from_secs(40),
            )
            .await?;
        assert_eq!(second_revoke, None);

        Ok(())
    });
}

#[test]
fn postgres_live_device_session_upsert_after_revoke_preserves_terminal_state() {
    run_live_postgres_test("device_session_upsert_terminal_guard", |pool| async move {
        seed_user(&pool, "tenant_ds_term", "user_ds_term").await?;

        let repository = PostgresDeviceSessionRepository::new(pool.clone());
        let base = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_748_300_000_000);
        let session = device_session(
            "tenant_ds_term",
            "user_ds_term",
            "session_ds_term",
            "okr_evidence",
            30,
            base,
        );
        repository
            .upsert_with_identity_hash(&session, "sha256:session-ds-term-01")
            .await?;

        let revoked = repository
            .revoke(
                "tenant_ds_term",
                "session_ds_term",
                base + std::time::Duration::from_secs(10),
            )
            .await?
            .expect("revoke should apply");
        let revoked_at = revoked.revoked_at.expect("revoked timestamp should be set");
        assert_eq!(revoked.state, SessionState::Revoked);

        let mut attempted_reactivation = session.clone();
        attempted_reactivation.cursor.value = 99;
        attempted_reactivation.last_seen_at = base + std::time::Duration::from_secs(30);
        attempted_reactivation.state = SessionState::Active;
        attempted_reactivation.revoked_at = None;

        let stored = repository
            .upsert_with_identity_hash(&attempted_reactivation, "sha256:session-ds-term-02")
            .await?;

        assert_eq!(stored.state, SessionState::Revoked);
        assert_eq!(stored.revoked_at, Some(revoked_at));
        assert_eq!(stored.sync_cursor_value, revoked.sync_cursor_value);
        assert_eq!(stored.session_identity_hash, "sha256:session-ds-term-01");

        Ok(())
    });
}

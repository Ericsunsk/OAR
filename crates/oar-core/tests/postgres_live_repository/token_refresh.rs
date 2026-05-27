use super::harness::*;

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
fn postgres_live_token_refresh_sweep_run_once_rotates_candidates_with_sequenced_audit() {
    run_live_postgres_test("token_refresh_sweep_run_once", |pool| async move {
        let due_before_ms = 1_748_550_000_000u64;
        let due_before = UNIX_EPOCH + std::time::Duration::from_millis(due_before_ms);
        let now = UNIX_EPOCH + std::time::Duration::from_millis(1_748_550_500_000);

        seed_user(&pool, "tenant_tr_sweep_success", "user_tr_sweep_success").await?;
        seed_identity(
            &pool,
            "tenant_tr_sweep_success",
            "identity_tr_sweep_success",
        )
        .await?;
        seed_user(&pool, "tenant_tr_sweep_other", "user_tr_sweep_other").await?;
        seed_identity(&pool, "tenant_tr_sweep_other", "identity_tr_sweep_other").await?;

        let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
        let mut expired = encrypted_token_grant_record(
            "tenant_tr_sweep_success",
            "grant_sweep_expired",
            "identity_tr_sweep_success",
            TokenGrantState::Expired,
            "fp-sweep-expired-old",
        );
        expired.expires_at_ms = Some(due_before_ms + 100_000);
        grant_repo.upsert_encrypted_grant(&expired).await?;

        let mut due_valid = encrypted_token_grant_record(
            "tenant_tr_sweep_success",
            "grant_sweep_due_valid",
            "identity_tr_sweep_success",
            TokenGrantState::Valid,
            "fp-sweep-valid-old",
        );
        due_valid.expires_at_ms = Some(due_before_ms - 1_000);
        grant_repo.upsert_encrypted_grant(&due_valid).await?;

        let mut future_valid = encrypted_token_grant_record(
            "tenant_tr_sweep_success",
            "grant_sweep_future_valid",
            "identity_tr_sweep_success",
            TokenGrantState::Valid,
            "fp-sweep-future-old",
        );
        future_valid.expires_at_ms = Some(due_before_ms + 86_400_000);
        grant_repo.upsert_encrypted_grant(&future_valid).await?;

        let mut other_tenant_due = encrypted_token_grant_record(
            "tenant_tr_sweep_other",
            "grant_sweep_other_tenant",
            "identity_tr_sweep_other",
            TokenGrantState::NeedsRefresh,
            "fp-sweep-other-old",
        );
        other_tenant_due.expires_at_ms = Some(due_before_ms - 2_000);
        grant_repo.upsert_encrypted_grant(&other_tenant_due).await?;

        let adapter = SequenceRefreshAdapter::new([
            RefreshOutcome::Success {
                rotated_material: EncryptedGrantMaterial {
                    encrypted_primary: vec![0xA1],
                    encrypted_renewal: vec![0xB1],
                },
                key_id: "key-sweep-v2-a".to_string(),
                new_fingerprint: "fp-sweep-expired-new".to_string(),
                refreshed_at: now,
                expires_at: Some(UNIX_EPOCH + std::time::Duration::from_millis(1_748_650_000_000)),
            },
            RefreshOutcome::Success {
                rotated_material: EncryptedGrantMaterial {
                    encrypted_primary: vec![0xA2],
                    encrypted_renewal: vec![0xB2],
                },
                key_id: "key-sweep-v2-b".to_string(),
                new_fingerprint: "fp-sweep-valid-new".to_string(),
                refreshed_at: now,
                expires_at: Some(UNIX_EPOCH + std::time::Duration::from_millis(1_748_660_000_000)),
            },
        ]);
        let mut sweep = PostgresTokenRefreshSweep::new(pool.clone(), adapter.clone());

        let report = sweep
            .run_once_for_tenant(PostgresTokenRefreshSweepRequest {
                tenant_id: "tenant_tr_sweep_success".to_string(),
                due_before,
                limit: 8,
                now,
                audit_trace_id: "trace_token_refresh_sweep_success".to_string(),
                audit_sequence_start: 51,
                occurred_at_ms: 1_748_550_500_111,
                actor: actor("user_tr_sweep_success"),
                workspace_id: Some("workspace_tr_sweep_success".to_string()),
            })
            .await?;

        assert_eq!(report.candidate_count, 2);
        assert_eq!(report.attempted_count, 2);
        assert!(!report.has_more);
        assert_eq!(report.reports.len(), 2);
        assert_eq!(
            adapter.called_grant_ids(),
            vec!["grant_sweep_expired", "grant_sweep_due_valid"]
        );
        assert!(report.reports.iter().all(|item| {
            item.service_report.status == TokenRefreshReportStatus::Succeeded
                && item.service_report.adapter_called
                && item.service_report.sink_called
                && item.event.trace_id == "trace_token_refresh_sweep_success"
        }));
        assert_eq!(report.reports[0].event.sequence, 51);
        assert_eq!(report.reports[1].event.sequence, 52);
        assert_eq!(
            report.reports[0].event.scope.workspace_id.as_deref(),
            Some("workspace_tr_sweep_success")
        );

        let expired_stored = grant_repo
            .get_by_id("tenant_tr_sweep_success", "grant_sweep_expired")
            .await?
            .expect("expired sweep grant should exist");
        assert_eq!(expired_stored.state, TokenGrantState::Valid);
        assert_eq!(
            expired_stored.oauth_grant_fingerprint,
            "fp-sweep-expired-new"
        );
        assert_eq!(expired_stored.oauth_grant_key_id, "key-sweep-v2-a");

        let valid_stored = grant_repo
            .get_by_id("tenant_tr_sweep_success", "grant_sweep_due_valid")
            .await?
            .expect("valid sweep grant should exist");
        assert_eq!(valid_stored.state, TokenGrantState::Valid);
        assert_eq!(valid_stored.oauth_grant_fingerprint, "fp-sweep-valid-new");
        assert_eq!(valid_stored.oauth_grant_key_id, "key-sweep-v2-b");

        let future_stored = grant_repo
            .get_by_id("tenant_tr_sweep_success", "grant_sweep_future_valid")
            .await?
            .expect("future sweep grant should exist");
        assert_eq!(future_stored.oauth_grant_fingerprint, "fp-sweep-future-old");

        let other_stored = grant_repo
            .get_by_id("tenant_tr_sweep_other", "grant_sweep_other_tenant")
            .await?
            .expect("other tenant sweep grant should exist");
        assert_eq!(other_stored.oauth_grant_fingerprint, "fp-sweep-other-old");

        let events = PostgresAuditEventRepository::new(pool.clone())
            .find_by_tenant_and_trace_id("tenant_tr_sweep", "trace_token_refresh_sweep_success")
            .await?;
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].sequence, 51);
        assert_eq!(events[1].sequence, 52);
        assert_eq!(events[0].target.action_type, "token_refresh.rotate");
        assert_eq!(events[1].target.action_type, "token_refresh.rotate");

        let payloads: Vec<serde_json::Value> = sqlx::query_scalar(
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
            ORDER BY sequence ASC
            "#,
        )
        .bind("tenant_tr_sweep")
        .bind("trace_token_refresh_sweep_success")
        .fetch_all(&pool)
        .await?;
        for payload in payloads {
            let payload_text = payload.to_string();
            assert_no_auth_refresh_sensitive_payload(&payload_text);
            assert!(!payload_text.contains("fp-sweep"));
            assert!(!payload_text.contains("encrypted"));
            assert!(!payload_text.contains("fingerprint"));
        }

        Ok(())
    });
}

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

#[test]
fn postgres_live_token_refresh_orchestrator_replaces_sync_sink_successfully() {
    run_live_postgres_test(
        "token_refresh_orchestrator_no_sync_sink_success",
        |pool| async move {
            seed_user(
                &pool,
                "tenant_tr_service_success",
                "user_tr_service_success",
            )
            .await?;
            seed_identity(
                &pool,
                "tenant_tr_service_success",
                "identity_tr_service_success",
            )
            .await?;

            let repository = PostgresTokenGrantRepository::new(pool.clone());
            let initial = encrypted_token_grant_record(
                "tenant_tr_service_success",
                "grant_tr_service_success",
                "identity_tr_service_success",
                TokenGrantState::NeedsRefresh,
                "fp-service-old",
            );
            repository.upsert_encrypted_grant(&initial).await?;

            let refreshed_at =
                SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_748_302_000_000);
            let adapter = LiveRefreshAdapter::new(RefreshOutcome::Success {
                rotated_material: EncryptedGrantMaterial {
                    encrypted_primary: vec![9, 9, 9],
                    encrypted_renewal: vec![8, 8, 8],
                },
                key_id: "key-v3".to_string(),
                new_fingerprint: "fp-service-new".to_string(),
                refreshed_at,
                expires_at: Some(
                    SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_748_402_000_000),
                ),
            });
            let mut orchestrator =
                PostgresTokenRefreshOrchestrator::new(pool.clone(), adapter.clone());

            let report = orchestrator
                .refresh_grant_with_audit(
                    TokenRefreshGrantSnapshot {
                        grant_id: TokenGrantId("grant_tr_service_success".to_string()),
                        tenant_id: TenantId("tenant_tr_service_success".to_string()),
                        expected_fingerprint: "fp-service-old".to_string(),
                        state: TokenGrantState::NeedsRefresh,
                        has_refresh_material: true,
                        revoked_at: None,
                        reauth_required_at: None,
                    },
                    refreshed_at,
                    TokenRefreshAuditContext {
                        trace_id: "trace_tr_service_success".to_string(),
                        sequence: 1,
                        occurred_at_ms: 1_748_302_000_001,
                        actor: actor("user_tr_service_success"),
                        workspace_id: None,
                    },
                )
                .await?;

            assert_eq!(
                report.service_report.status,
                TokenRefreshReportStatus::Succeeded
            );
            assert!(report.service_report.adapter_called);
            assert!(report.service_report.sink_called);
            assert_eq!(orchestrator.adapter().calls(), 1);
            let report_debug = format!("{:?}", report.service_report);
            let audit_debug = format!("{:?}", report.service_report.audit_summary());
            assert!(!report_debug.contains("9, 9, 9"));
            assert!(!report_debug.contains("8, 8, 8"));
            assert!(!audit_debug.contains("9, 9, 9"));
            assert!(!audit_debug.contains("8, 8, 8"));
            assert_eq!(report.event.target.action_type, "token_refresh.rotate");

            let updated = repository
                .get_by_id("tenant_tr_service_success", "grant_tr_service_success")
                .await?
                .expect("token grant should exist after rotation");
            assert_eq!(updated.state, TokenGrantState::Valid);
            assert_eq!(updated.oauth_grant_fingerprint, "fp-service-new");
            assert_eq!(updated.oauth_grant_key_id, "key-v3");
            assert_eq!(
                updated.encrypted_oauth_grant,
                vec![0, 0, 0, 3, 9, 9, 9, 0, 0, 0, 3, 8, 8, 8]
            );

            Ok(())
        },
    );
}

#[test]
fn postgres_live_token_refresh_orchestrator_replaces_sync_sink_stale_fingerprint_noop() {
    run_live_postgres_test(
        "token_refresh_orchestrator_no_sync_sink_stale_fp",
        |pool| async move {
            seed_user(&pool, "tenant_tr_service_noop", "user_tr_service_noop").await?;
            seed_identity(&pool, "tenant_tr_service_noop", "identity_tr_service_noop").await?;

            let repository = PostgresTokenGrantRepository::new(pool.clone());
            let initial = encrypted_token_grant_record(
                "tenant_tr_service_noop",
                "grant_tr_service_noop",
                "identity_tr_service_noop",
                TokenGrantState::NeedsRefresh,
                "fp-current",
            );
            repository.upsert_encrypted_grant(&initial).await?;

            let now = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_748_303_000_000);
            let adapter = LiveRefreshAdapter::new(RefreshOutcome::Success {
                rotated_material: EncryptedGrantMaterial {
                    encrypted_primary: vec![9, 9, 9],
                    encrypted_renewal: vec![8, 8, 8],
                },
                key_id: "key-v4".to_string(),
                new_fingerprint: "fp-noop-new".to_string(),
                refreshed_at: now,
                expires_at: None,
            });
            let mut orchestrator =
                PostgresTokenRefreshOrchestrator::new(pool.clone(), adapter.clone());

            let report = orchestrator
                .refresh_grant_with_audit(
                    TokenRefreshGrantSnapshot {
                        grant_id: TokenGrantId("grant_tr_service_noop".to_string()),
                        tenant_id: TenantId("tenant_tr_service_noop".to_string()),
                        expected_fingerprint: "fp-stale".to_string(),
                        state: TokenGrantState::NeedsRefresh,
                        has_refresh_material: true,
                        revoked_at: None,
                        reauth_required_at: None,
                    },
                    now,
                    TokenRefreshAuditContext {
                        trace_id: "trace_tr_service_noop".to_string(),
                        sequence: 1,
                        occurred_at_ms: 1_748_303_000_001,
                        actor: actor("user_tr_service_noop"),
                        workspace_id: None,
                    },
                )
                .await?;

            assert_eq!(
                report.service_report.status,
                TokenRefreshReportStatus::ConflictNoop
            );
            assert!(report.service_report.adapter_called);
            assert!(report.service_report.sink_called);
            assert_eq!(orchestrator.adapter().calls(), 1);
            assert_eq!(report.event.event_type, AuditEventType::ExecutionFailed);
            let report_debug = format!("{:?}", report.service_report);
            assert!(!report_debug.contains("9, 9, 9"));
            assert!(!report_debug.contains("8, 8, 8"));

            let stored = repository
                .get_by_id("tenant_tr_service_noop", "grant_tr_service_noop")
                .await?
                .expect("token grant should remain after stale fingerprint noop");
            assert_eq!(stored.state, TokenGrantState::NeedsRefresh);
            assert_eq!(stored.oauth_grant_fingerprint, "fp-current");
            assert_eq!(stored.oauth_grant_key_id, "key-v1");
            assert_eq!(stored.encrypted_oauth_grant, vec![0x01, 0x02, 0x03]);

            Ok(())
        },
    );
}

#[test]
fn postgres_live_token_refresh_orchestrator_rotate_success() {
    run_live_postgres_test("token_refresh_orchestrator_success", |pool| async move {
        seed_user(&pool, "tenant_tr_orch_success", "user_tr_orch_success").await?;
        seed_identity(&pool, "tenant_tr_orch_success", "identity_tr_orch_success").await?;

        let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
        grant_repo
            .upsert_encrypted_grant(&encrypted_token_grant_record(
                "tenant_tr_orch_success",
                "grant_tr_orch_success",
                "identity_tr_orch_success",
                TokenGrantState::NeedsRefresh,
                "fp-orch-old",
            ))
            .await?;

        let refreshed_at =
            SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_748_510_000_000);
        let mut orchestrator = PostgresTokenRefreshOrchestrator::new(
            pool.clone(),
            LiveRefreshAdapter::new(RefreshOutcome::Success {
                rotated_material: EncryptedGrantMaterial {
                    encrypted_primary: vec![9, 9, 9],
                    encrypted_renewal: vec![8, 8, 8],
                },
                key_id: "key-orch-v2".to_string(),
                new_fingerprint: "fp-orch-new".to_string(),
                refreshed_at,
                expires_at: None,
            }),
        );

        let report = orchestrator
            .refresh_grant_with_audit(
                TokenRefreshGrantSnapshot {
                    grant_id: TokenGrantId("grant_tr_orch_success".to_string()),
                    tenant_id: TenantId("tenant_tr_orch_success".to_string()),
                    expected_fingerprint: "fp-orch-old".to_string(),
                    state: TokenGrantState::NeedsRefresh,
                    has_refresh_material: true,
                    revoked_at: None,
                    reauth_required_at: None,
                },
                refreshed_at,
                TokenRefreshAuditContext {
                    trace_id: "trace_token_refresh_orch_success".to_string(),
                    sequence: 21,
                    occurred_at_ms: 1_748_510_000_111,
                    actor: actor("user_tr_orch_success"),
                    workspace_id: None,
                },
            )
            .await?;

        assert_eq!(
            report.service_report.status,
            TokenRefreshReportStatus::Succeeded
        );
        assert_eq!(orchestrator.adapter().calls(), 1);
        assert_eq!(report.event.target.action_type, "token_refresh.rotate");

        let stored = grant_repo
            .get_by_id("tenant_tr_orch_success", "grant_tr_orch_success")
            .await?
            .expect("grant should exist");
        assert_eq!(stored.state, TokenGrantState::Valid);
        assert_eq!(stored.oauth_grant_fingerprint, "fp-orch-new");
        assert_eq!(stored.oauth_grant_key_id, "key-orch-v2");

        Ok(())
    });
}

#[test]
fn postgres_live_token_refresh_orchestrator_with_lark_auth_fixture_rotates_successfully() {
    run_live_postgres_test(
        "token_refresh_orchestrator_lark_fixture_rotate",
        |pool| async move {
            seed_user(
                &pool,
                "tenant_tr_orch_lark_fixture_rotate",
                "user_tr_orch_lark_fixture_rotate",
            )
            .await?;
            seed_identity(
                &pool,
                "tenant_tr_orch_lark_fixture_rotate",
                "identity_tr_orch_lark_fixture_rotate",
            )
            .await?;

            let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
            grant_repo
                .upsert_encrypted_grant(&encrypted_token_grant_record(
                    "tenant_tr_orch_lark_fixture_rotate",
                    "grant_tr_orch_lark_fixture_rotate",
                    "identity_tr_orch_lark_fixture_rotate",
                    TokenGrantState::NeedsRefresh,
                    "fp_prev_v1",
                ))
                .await?;

            let client = FixtureClient::new(AUTH_REFRESH_ROTATED_ENCRYPTED_JSON);
            let mut orchestrator = PostgresTokenRefreshOrchestrator::new(
                pool.clone(),
                LarkAuthRefreshAdapter::new(client.clone()),
            );

            let now = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_779_465_600_000);
            let report = orchestrator
                .refresh_grant_with_audit(
                    TokenRefreshGrantSnapshot {
                        grant_id: TokenGrantId("grant_tr_orch_lark_fixture_rotate".to_string()),
                        tenant_id: TenantId("tenant_tr_orch_lark_fixture_rotate".to_string()),
                        expected_fingerprint: "fp_prev_v1".to_string(),
                        state: TokenGrantState::NeedsRefresh,
                        has_refresh_material: true,
                        revoked_at: None,
                        reauth_required_at: None,
                    },
                    now,
                    TokenRefreshAuditContext {
                        trace_id: "trace_token_refresh_orch_lark_fixture_rotate".to_string(),
                        sequence: 31,
                        occurred_at_ms: 1_779_465_600_111,
                        actor: actor("user_tr_orch_lark_fixture_rotate"),
                        workspace_id: None,
                    },
                )
                .await?;

            assert_eq!(
                report.service_report.status,
                TokenRefreshReportStatus::Succeeded
            );
            assert_eq!(report.event.target.action_type, "token_refresh.rotate");
            assert_eq!(client.calls(), 1);

            let stored = grant_repo
                .get_by_id(
                    "tenant_tr_orch_lark_fixture_rotate",
                    "grant_tr_orch_lark_fixture_rotate",
                )
                .await?
                .expect("grant should exist");
            assert_eq!(stored.state, TokenGrantState::Valid);
            assert_eq!(stored.oauth_grant_fingerprint, "fp_rotated_v2");
            assert_eq!(stored.oauth_grant_key_id, "kms-key-2026-05");
            assert_ne!(stored.encrypted_oauth_grant, vec![0x01, 0x02, 0x03]);

            let payload: serde_json::Value = sqlx::query_scalar(
                r#"
            SELECT
            jsonb_build_object(
              'before_summary', before_summary,
              'after_summary', after_summary,
              'execution_result', execution_result
            )
            FROM audit_events
            WHERE event_id = $1
            "#,
            )
            .bind(&report.event.event_id)
            .fetch_one(&pool)
            .await?;
            let payload_text = payload.to_string();
            assert_no_auth_refresh_sensitive_payload(&payload_text);

            Ok(())
        },
    );
}

#[test]
fn postgres_live_token_refresh_orchestrator_with_lark_auth_reauth_marks_reauth_required() {
    run_live_postgres_test(
        "token_refresh_orchestrator_lark_fixture_reauth",
        |pool| async move {
            seed_user(
                &pool,
                "tenant_tr_orch_lark_fixture_reauth",
                "user_tr_orch_lark_fixture_reauth",
            )
            .await?;
            seed_identity(
                &pool,
                "tenant_tr_orch_lark_fixture_reauth",
                "identity_tr_orch_lark_fixture_reauth",
            )
            .await?;

            let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
            grant_repo
                .upsert_encrypted_grant(&encrypted_token_grant_record(
                    "tenant_tr_orch_lark_fixture_reauth",
                    "grant_tr_orch_lark_fixture_reauth",
                    "identity_tr_orch_lark_fixture_reauth",
                    TokenGrantState::Valid,
                    "fp_prev_v1",
                ))
                .await?;

            let client = FixtureClient::new(AUTH_REFRESH_REAUTH_REQUIRED_JSON);
            let mut orchestrator = PostgresTokenRefreshOrchestrator::new(
                pool.clone(),
                LarkAuthRefreshAdapter::new(client.clone()),
            );

            let now = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_779_465_700_000);
            let report = orchestrator
                .refresh_grant_with_audit(
                    TokenRefreshGrantSnapshot {
                        grant_id: TokenGrantId("grant_tr_orch_lark_fixture_reauth".to_string()),
                        tenant_id: TenantId("tenant_tr_orch_lark_fixture_reauth".to_string()),
                        expected_fingerprint: "fp_prev_v1".to_string(),
                        state: TokenGrantState::Valid,
                        has_refresh_material: true,
                        revoked_at: None,
                        reauth_required_at: None,
                    },
                    now,
                    TokenRefreshAuditContext {
                        trace_id: "trace_token_refresh_orch_lark_fixture_reauth".to_string(),
                        sequence: 32,
                        occurred_at_ms: 1_779_465_700_111,
                        actor: actor("user_tr_orch_lark_fixture_reauth"),
                        workspace_id: None,
                    },
                )
                .await?;

            assert_eq!(
                report.service_report.status,
                TokenRefreshReportStatus::Succeeded
            );
            assert_eq!(
                report.service_report.safe_error.as_deref(),
                Some("invalid_grant")
            );
            assert_eq!(
                report.event.target.action_type,
                "token_refresh.mark_reauth_required"
            );
            assert_eq!(client.calls(), 1);

            let stored = grant_repo
                .get_by_id(
                    "tenant_tr_orch_lark_fixture_reauth",
                    "grant_tr_orch_lark_fixture_reauth",
                )
                .await?
                .expect("grant should exist");
            assert_eq!(stored.state, TokenGrantState::ReauthRequired);
            assert_eq!(stored.last_refresh_error.as_deref(), Some("invalid_grant"));

            let payload: serde_json::Value = sqlx::query_scalar(
                r#"
            SELECT
            jsonb_build_object(
              'before_summary', before_summary,
              'after_summary', after_summary,
              'execution_result', execution_result
            )
            FROM audit_events
            WHERE event_id = $1
            "#,
            )
            .bind(&report.event.event_id)
            .fetch_one(&pool)
            .await?;
            assert_no_auth_refresh_sensitive_payload(&payload.to_string());

            Ok(())
        },
    );
}

#[test]
fn postgres_live_token_refresh_orchestrator_with_lark_auth_plaintext_fixture_is_safe_transient() {
    run_live_postgres_test(
        "token_refresh_orchestrator_lark_fixture_plaintext",
        |pool| async move {
            seed_user(
                &pool,
                "tenant_tr_orch_lark_fixture_plaintext",
                "user_tr_orch_lark_fixture_plaintext",
            )
            .await?;
            seed_identity(
                &pool,
                "tenant_tr_orch_lark_fixture_plaintext",
                "identity_tr_orch_lark_fixture_plaintext",
            )
            .await?;

            let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
            grant_repo
                .upsert_encrypted_grant(&encrypted_token_grant_record(
                    "tenant_tr_orch_lark_fixture_plaintext",
                    "grant_tr_orch_lark_fixture_plaintext",
                    "identity_tr_orch_lark_fixture_plaintext",
                    TokenGrantState::Valid,
                    "fp_prev_v1",
                ))
                .await?;

            let client = FixtureClient::new(AUTH_REFRESH_PLAINTEXT_TOKEN_LEAK_JSON);
            let mut orchestrator = PostgresTokenRefreshOrchestrator::new(
                pool.clone(),
                LarkAuthRefreshAdapter::new(client.clone()),
            );

            let now = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_779_465_800_000);
            let report = orchestrator
                .refresh_grant_with_audit(
                    TokenRefreshGrantSnapshot {
                        grant_id: TokenGrantId("grant_tr_orch_lark_fixture_plaintext".to_string()),
                        tenant_id: TenantId("tenant_tr_orch_lark_fixture_plaintext".to_string()),
                        expected_fingerprint: "fp_prev_v1".to_string(),
                        state: TokenGrantState::Valid,
                        has_refresh_material: true,
                        revoked_at: None,
                        reauth_required_at: None,
                    },
                    now,
                    TokenRefreshAuditContext {
                        trace_id: "trace_token_refresh_orch_lark_fixture_plaintext".to_string(),
                        sequence: 33,
                        occurred_at_ms: 1_779_465_800_111,
                        actor: actor("user_tr_orch_lark_fixture_plaintext"),
                        workspace_id: None,
                    },
                )
                .await?;

            assert_eq!(
                report.service_report.status,
                TokenRefreshReportStatus::Succeeded
            );
            assert_eq!(
                report.service_report.safe_error.as_deref(),
                Some("temporarily unavailable")
            );
            assert_eq!(client.calls(), 1);

            let stored = grant_repo
                .get_by_id(
                    "tenant_tr_orch_lark_fixture_plaintext",
                    "grant_tr_orch_lark_fixture_plaintext",
                )
                .await?
                .expect("grant should exist");
            assert_eq!(stored.state, TokenGrantState::NeedsRefresh);
            assert_eq!(
                stored.last_refresh_error.as_deref(),
                Some("temporarily unavailable")
            );

            let payload: serde_json::Value = sqlx::query_scalar(
                r#"
                SELECT
            jsonb_build_object(
              'before_summary', before_summary,
              'after_summary', after_summary,
              'execution_result', execution_result
            )
            FROM audit_events
                WHERE event_id = $1
                "#,
            )
            .bind(&report.event.event_id)
            .fetch_one(&pool)
            .await?;
            let payload_text = payload.to_string();
            assert_no_auth_refresh_sensitive_payload(&payload_text);
            assert!(!payload_text.contains("tok_access_live_should_never_parse"));
            assert!(!payload_text.contains("tok_refresh_live_should_never_parse"));
            assert!(!payload_text.contains("refresh_token="));
            assert!(!payload_text.contains("access_token="));

            Ok(())
        },
    );
}

#[test]
fn postgres_live_token_refresh_orchestrator_stale_conflict_noop() {
    run_live_postgres_test("token_refresh_orchestrator_stale", |pool| async move {
        seed_user(&pool, "tenant_tr_orch_noop", "user_tr_orch_noop").await?;
        seed_identity(&pool, "tenant_tr_orch_noop", "identity_tr_orch_noop").await?;

        let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
        grant_repo
            .upsert_encrypted_grant(&encrypted_token_grant_record(
                "tenant_tr_orch_noop",
                "grant_tr_orch_noop",
                "identity_tr_orch_noop",
                TokenGrantState::NeedsRefresh,
                "fp-current",
            ))
            .await?;

        let now = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_748_520_000_000);
        let mut orchestrator = PostgresTokenRefreshOrchestrator::new(
            pool.clone(),
            LiveRefreshAdapter::new(RefreshOutcome::Success {
                rotated_material: EncryptedGrantMaterial {
                    encrypted_primary: vec![9, 9, 9],
                    encrypted_renewal: vec![8, 8, 8],
                },
                key_id: "key-orch-v2".to_string(),
                new_fingerprint: "fp-orch-noop-new".to_string(),
                refreshed_at: now,
                expires_at: None,
            }),
        );

        let report = orchestrator
            .refresh_grant_with_audit(
                TokenRefreshGrantSnapshot {
                    grant_id: TokenGrantId("grant_tr_orch_noop".to_string()),
                    tenant_id: TenantId("tenant_tr_orch_noop".to_string()),
                    expected_fingerprint: "fp-stale".to_string(),
                    state: TokenGrantState::NeedsRefresh,
                    has_refresh_material: true,
                    revoked_at: None,
                    reauth_required_at: None,
                },
                now,
                TokenRefreshAuditContext {
                    trace_id: "trace_token_refresh_orch_noop".to_string(),
                    sequence: 22,
                    occurred_at_ms: 1_748_520_000_111,
                    actor: actor("user_tr_orch_noop"),
                    workspace_id: None,
                },
            )
            .await?;

        assert_eq!(
            report.service_report.status,
            TokenRefreshReportStatus::ConflictNoop
        );
        assert_eq!(orchestrator.adapter().calls(), 1);
        assert_eq!(report.event.event_type, AuditEventType::ExecutionFailed);
        assert_eq!(
            report
                .event
                .execution
                .as_ref()
                .and_then(|execution| execution.error_code.as_deref()),
            Some("token_refresh_conflict_noop")
        );

        let stored = grant_repo
            .get_by_id("tenant_tr_orch_noop", "grant_tr_orch_noop")
            .await?
            .expect("grant should remain");
        assert_eq!(stored.state, TokenGrantState::NeedsRefresh);
        assert_eq!(stored.oauth_grant_fingerprint, "fp-current");

        Ok(())
    });
}

#[test]
fn postgres_live_token_refresh_orchestrator_transient_failure_redacts() {
    run_live_postgres_test("token_refresh_orchestrator_redacts", |pool| async move {
        seed_user(&pool, "tenant_tr_orch_redact", "user_tr_orch_redact").await?;
        seed_identity(&pool, "tenant_tr_orch_redact", "identity_tr_orch_redact").await?;

        let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
        grant_repo
            .upsert_encrypted_grant(&encrypted_token_grant_record(
                "tenant_tr_orch_redact",
                "grant_tr_orch_redact",
                "identity_tr_orch_redact",
                TokenGrantState::Valid,
                "fp-orch-redact",
            ))
            .await?;

        let now = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_748_530_000_000);
        let raw = "refresh_token=rt_fake Authorization: Bearer at_fake";
        let mut orchestrator = PostgresTokenRefreshOrchestrator::new(
            pool.clone(),
            LiveRefreshAdapter::new(RefreshOutcome::TransientFailure {
                safe_error: raw.to_string(),
            }),
        );

        let report = orchestrator
            .refresh_grant_with_audit(
                TokenRefreshGrantSnapshot {
                    grant_id: TokenGrantId("grant_tr_orch_redact".to_string()),
                    tenant_id: TenantId("tenant_tr_orch_redact".to_string()),
                    expected_fingerprint: "fp-orch-redact".to_string(),
                    state: TokenGrantState::Valid,
                    has_refresh_material: true,
                    revoked_at: None,
                    reauth_required_at: None,
                },
                now,
                TokenRefreshAuditContext {
                    trace_id: "trace_token_refresh_orch_redact".to_string(),
                    sequence: 23,
                    occurred_at_ms: 1_748_530_000_111,
                    actor: actor("user_tr_orch_redact"),
                    workspace_id: None,
                },
            )
            .await?;

        assert_eq!(
            report.service_report.status,
            TokenRefreshReportStatus::Succeeded
        );
        assert_eq!(
            report.service_report.safe_error.as_deref(),
            Some("<redacted refresh error>")
        );
        assert_eq!(orchestrator.adapter().calls(), 1);

        let stored = grant_repo
            .get_by_id("tenant_tr_orch_redact", "grant_tr_orch_redact")
            .await?
            .expect("grant should remain");
        assert_eq!(
            stored.last_refresh_error.as_deref(),
            Some("<redacted refresh error>")
        );

        let payload: serde_json::Value = sqlx::query_scalar(
            r#"
            SELECT
            jsonb_build_object(
              'before_summary', before_summary,
              'after_summary', after_summary,
              'execution_result', execution_result
            )
            FROM audit_events
            WHERE event_id = $1
            "#,
        )
        .bind(&report.event.event_id)
        .fetch_one(&pool)
        .await?;
        let payload_text = payload.to_string();
        assert!(!payload_text.contains("refresh_token=rt_fake"));
        assert!(!payload_text.contains("Bearer at_fake"));
        assert!(!payload_text.contains(raw));

        Ok(())
    });
}

#[test]
fn postgres_live_token_refresh_orchestrator_short_circuit_revoked() {
    run_live_postgres_test(
        "token_refresh_orchestrator_short_circuit",
        |pool| async move {
            seed_user(&pool, "tenant_tr_orch_short", "user_tr_orch_short").await?;
            seed_identity(&pool, "tenant_tr_orch_short", "identity_tr_orch_short").await?;

            let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
            grant_repo
                .upsert_encrypted_grant(&encrypted_token_grant_record(
                    "tenant_tr_orch_short",
                    "grant_tr_orch_short",
                    "identity_tr_orch_short",
                    TokenGrantState::Valid,
                    "fp-short",
                ))
                .await?;

            let now = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_748_540_000_000);
            let mut orchestrator = PostgresTokenRefreshOrchestrator::new(
                pool.clone(),
                LiveRefreshAdapter::new(RefreshOutcome::Success {
                    rotated_material: EncryptedGrantMaterial {
                        encrypted_primary: vec![9, 9, 9],
                        encrypted_renewal: vec![8, 8, 8],
                    },
                    key_id: "key-never-used".to_string(),
                    new_fingerprint: "fp-never-used".to_string(),
                    refreshed_at: now,
                    expires_at: None,
                }),
            );

            let report = orchestrator
                .refresh_grant_with_audit(
                    TokenRefreshGrantSnapshot {
                        grant_id: TokenGrantId("grant_tr_orch_short".to_string()),
                        tenant_id: TenantId("tenant_tr_orch_short".to_string()),
                        expected_fingerprint: "fp-short".to_string(),
                        state: TokenGrantState::Revoked,
                        has_refresh_material: true,
                        revoked_at: Some(now),
                        reauth_required_at: None,
                    },
                    now,
                    TokenRefreshAuditContext {
                        trace_id: "trace_token_refresh_orch_short".to_string(),
                        sequence: 24,
                        occurred_at_ms: 1_748_540_000_111,
                        actor: actor("user_tr_orch_short"),
                        workspace_id: None,
                    },
                )
                .await?;

            assert_eq!(
                report.service_report.status,
                TokenRefreshReportStatus::ShortCircuited(
                    oar_core::domain::token_refresh::types::TokenRefreshShortCircuitReason::Revoked
                )
            );
            assert_eq!(orchestrator.adapter().calls(), 0);
            assert_eq!(report.event.event_type, AuditEventType::ExecutionDenied);

            let stored = grant_repo
                .get_by_id("tenant_tr_orch_short", "grant_tr_orch_short")
                .await?
                .expect("grant should remain");
            assert_eq!(stored.oauth_grant_fingerprint, "fp-short");
            assert_eq!(stored.state, TokenGrantState::Valid);

            Ok(())
        },
    );
}

#[test]
fn postgres_live_token_refresh_uow_rotate_success() {
    run_live_postgres_test("token_refresh_uow_rotate_success", |pool| async move {
        seed_user(&pool, "tenant_tr_uow_success", "user_tr_uow_success").await?;
        seed_identity(&pool, "tenant_tr_uow_success", "identity_tr_uow_success").await?;

        let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
        grant_repo
            .upsert_encrypted_grant(&encrypted_token_grant_record(
                "tenant_tr_uow_success",
                "grant_tr_uow_success",
                "identity_tr_uow_success",
                TokenGrantState::NeedsRefresh,
                "fp-uow-old",
            ))
            .await?;

        let uow = PostgresTokenRefreshUnitOfWork::new(pool.clone());
        let report = uow
            .apply_planned_command_with_audit(
                planned_token_refresh_command(TokenRefreshRepositoryCommand::RotateGrantCas {
                    grant_id: TokenGrantId("grant_tr_uow_success".to_string()),
                    tenant_id: TenantId("tenant_tr_uow_success".to_string()),
                    expected_fingerprint: "fp-uow-old".to_string(),
                    expires_at_ms: Some(1_748_480_000_000),
                    refreshed_at_ms: 1_748_470_000_000,
                    encrypted_grant_blob: EncryptedGrantBlob(vec![0x11, 0x22, 0x33]),
                    grant_key_id: "key-uow-v2".to_string(),
                    new_fingerprint: "fp-uow-new".to_string(),
                }),
                TokenRefreshAuditContext {
                    trace_id: "trace_token_refresh_uow_success".to_string(),
                    sequence: 11,
                    occurred_at_ms: 1_748_470_000_001,
                    actor: actor("user_tr_uow_success"),
                    workspace_id: None,
                },
            )
            .await?;

        let apply_result = report.apply_result.expect("rotate should apply");
        assert_eq!(apply_result.grant_id.0, "grant_tr_uow_success");
        assert_eq!(apply_result.tenant_id.0, "tenant_tr_uow_success");
        assert_eq!(apply_result.state, TokenGrantState::Valid);
        assert_eq!(report.event.target.action_type, "token_refresh.rotate");

        let stored = grant_repo
            .get_by_id("tenant_tr_uow_success", "grant_tr_uow_success")
            .await?
            .expect("grant should exist");
        assert_eq!(stored.oauth_grant_fingerprint, "fp-uow-new");
        assert_eq!(stored.state, TokenGrantState::Valid);

        let events = PostgresAuditEventRepository::new(pool.clone())
            .find_by_tenant_and_trace_id("tenant_tr_uow", "trace_token_refresh_uow_success")
            .await?;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].target.action_type, "token_refresh.rotate");

        let payload: serde_json::Value = sqlx::query_scalar(
            r#"
            SELECT
            jsonb_build_object(
              'before_summary', before_summary,
              'after_summary', after_summary,
              'execution_result', execution_result
            )
            FROM audit_events
            WHERE event_id = $1
            "#,
        )
        .bind(&report.event.event_id)
        .fetch_one(&pool)
        .await?;
        let payload_text = payload.to_string().to_lowercase();
        assert!(!payload_text.contains("access_token"));
        assert!(!payload_text.contains("refresh_token"));
        assert!(!payload_text.contains("authorization"));
        assert!(!payload_text.contains("fingerprint"));
        assert!(!payload_text.contains("encrypted"));
        assert!(!payload_text.contains("9, 9, 9"));

        Ok(())
    });
}

#[test]
fn postgres_live_token_refresh_uow_stale_fingerprint_conflict_noop() {
    run_live_postgres_test("token_refresh_uow_stale_fingerprint", |pool| async move {
        seed_user(&pool, "tenant_tr_uow_noop", "user_tr_uow_noop").await?;
        seed_identity(&pool, "tenant_tr_uow_noop", "identity_tr_uow_noop").await?;

        let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
        grant_repo
            .upsert_encrypted_grant(&encrypted_token_grant_record(
                "tenant_tr_uow_noop",
                "grant_tr_uow_noop",
                "identity_tr_uow_noop",
                TokenGrantState::NeedsRefresh,
                "fp-current",
            ))
            .await?;

        let uow = PostgresTokenRefreshUnitOfWork::new(pool.clone());
        let report = uow
            .apply_planned_command_with_audit(
                planned_token_refresh_command(TokenRefreshRepositoryCommand::RotateGrantCas {
                    grant_id: TokenGrantId("grant_tr_uow_noop".to_string()),
                    tenant_id: TenantId("tenant_tr_uow_noop".to_string()),
                    expected_fingerprint: "fp-stale".to_string(),
                    expires_at_ms: Some(1_748_490_000_000),
                    refreshed_at_ms: 1_748_480_000_000,
                    encrypted_grant_blob: EncryptedGrantBlob(vec![9, 9, 9]),
                    grant_key_id: "key-uow-v2".to_string(),
                    new_fingerprint: "fp-noop-new".to_string(),
                }),
                TokenRefreshAuditContext {
                    trace_id: "trace_token_refresh_uow_noop".to_string(),
                    sequence: 12,
                    occurred_at_ms: 1_748_480_000_001,
                    actor: actor("user_tr_uow_noop"),
                    workspace_id: None,
                },
            )
            .await?;

        assert_eq!(report.apply_result, None);
        assert_eq!(report.event.event_type, AuditEventType::ExecutionFailed);
        assert_eq!(
            report
                .event
                .execution
                .as_ref()
                .and_then(|execution| execution.error_code.as_deref()),
            Some("token_refresh_conflict_noop")
        );

        let stored = grant_repo
            .get_by_id("tenant_tr_uow_noop", "grant_tr_uow_noop")
            .await?
            .expect("grant should remain");
        assert_eq!(stored.oauth_grant_fingerprint, "fp-current");
        assert_eq!(stored.oauth_grant_key_id, "key-v1");
        assert_eq!(stored.encrypted_oauth_grant, vec![0x01, 0x02, 0x03]);

        let events = PostgresAuditEventRepository::new(pool.clone())
            .find_by_tenant_and_trace_id("tenant_tr_uow_noop", "trace_token_refresh_uow_noop")
            .await?;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, AuditEventType::ExecutionFailed);
        assert_eq!(
            events[0]
                .execution
                .as_ref()
                .and_then(|execution| execution.error_code.as_deref()),
            Some("token_refresh_conflict_noop")
        );

        let payload: serde_json::Value = sqlx::query_scalar(
            r#"
            SELECT
            jsonb_build_object(
              'before_summary', before_summary,
              'after_summary', after_summary,
              'execution_result', execution_result
            )
            FROM audit_events
            WHERE event_id = $1
            "#,
        )
        .bind(&report.event.event_id)
        .fetch_one(&pool)
        .await?;
        let payload_text = payload.to_string().to_lowercase();
        assert!(!payload_text.contains("access_token"));
        assert!(!payload_text.contains("refresh_token"));
        assert!(!payload_text.contains("authorization"));
        assert!(!payload_text.contains("fingerprint"));
        assert!(!payload_text.contains("encrypted"));
        assert!(!payload_text.contains("9, 9, 9"));

        Ok(())
    });
}

#[test]
fn postgres_live_token_refresh_uow_mark_needs_refresh_redacts_audit_error() {
    run_live_postgres_test("token_refresh_uow_mark_needs_redacts", |pool| async move {
        seed_user(
            &pool,
            "tenant_tr_uow_needs_redact",
            "user_tr_uow_needs_redact",
        )
        .await?;
        seed_identity(
            &pool,
            "tenant_tr_uow_needs_redact",
            "identity_tr_uow_needs_redact",
        )
        .await?;

        let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
        grant_repo
            .upsert_encrypted_grant(&encrypted_token_grant_record(
                "tenant_tr_uow_needs_redact",
                "grant_tr_uow_needs_redact",
                "identity_tr_uow_needs_redact",
                TokenGrantState::Valid,
                "fp-uow-needs-redact",
            ))
            .await?;

        let report = PostgresTokenRefreshUnitOfWork::new(pool.clone())
            .apply_planned_command_with_audit(
                planned_token_refresh_command(TokenRefreshRepositoryCommand::MarkNeedsRefresh {
                    grant_id: TokenGrantId("grant_tr_uow_needs_redact".to_string()),
                    tenant_id: TenantId("tenant_tr_uow_needs_redact".to_string()),
                    expected_fingerprint: "fp-uow-needs-redact".to_string(),
                    refreshed_at_ms: 1_748_485_000_000,
                    safe_error: "refresh_token=rt_fake Authorization: Bearer at_fake".to_string(),
                }),
                TokenRefreshAuditContext {
                    trace_id: "trace_token_refresh_uow_needs_redact".to_string(),
                    sequence: 13,
                    occurred_at_ms: 1_748_485_000_001,
                    actor: actor("user_tr_uow_needs_redact"),
                    workspace_id: None,
                },
            )
            .await?;

        let updated = grant_repo
            .get_by_id("tenant_tr_uow_needs_redact", "grant_tr_uow_needs_redact")
            .await?
            .expect("grant should exist after needs-refresh mark");
        assert_eq!(updated.state, TokenGrantState::NeedsRefresh);
        assert_eq!(
            updated.last_refresh_error.as_deref(),
            Some("<redacted refresh error>")
        );

        let payload: serde_json::Value = sqlx::query_scalar(
            r#"
            SELECT
            jsonb_build_object(
              'before_summary', before_summary,
              'after_summary', after_summary,
              'execution_result', execution_result
            )
            FROM audit_events
            WHERE event_id = $1
            "#,
        )
        .bind(&report.event.event_id)
        .fetch_one(&pool)
        .await?;
        let payload_text = payload.to_string().to_lowercase();
        assert!(payload_text.contains("<redacted refresh error>"));
        assert!(!payload_text.contains("refresh_token"));
        assert!(!payload_text.contains("authorization"));
        assert!(!payload_text.contains("bearer"));
        assert!(!payload_text.contains("rt_fake"));
        assert!(!payload_text.contains("at_fake"));

        Ok(())
    });
}

#[test]
fn postgres_live_token_refresh_uow_rejects_mismatched_plan_without_mutation() {
    run_live_postgres_test("token_refresh_uow_plan_mismatch", |pool| async move {
        seed_user(&pool, "tenant_tr_uow_mismatch", "user_tr_uow_mismatch").await?;
        seed_identity(&pool, "tenant_tr_uow_mismatch", "identity_tr_uow_mismatch").await?;

        let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
        grant_repo
            .upsert_encrypted_grant(&encrypted_token_grant_record(
                "tenant_tr_uow_mismatch",
                "grant_tr_uow_mismatch",
                "identity_tr_uow_mismatch",
                TokenGrantState::Valid,
                "fp-uow-mismatch",
            ))
            .await?;

        let mut planned =
            planned_token_refresh_command(TokenRefreshRepositoryCommand::MarkNeedsRefresh {
                grant_id: TokenGrantId("grant_tr_uow_mismatch".to_string()),
                tenant_id: TenantId("tenant_tr_uow_mismatch".to_string()),
                expected_fingerprint: "fp-uow-mismatch".to_string(),
                refreshed_at_ms: 1_748_486_000_000,
                safe_error: "temporarily unavailable".to_string(),
            });
        planned.report.tenant_id = TenantId("tenant_tr_uow_other".to_string());

        let result = PostgresTokenRefreshUnitOfWork::new(pool.clone())
            .apply_planned_command_with_audit(
                planned,
                TokenRefreshAuditContext {
                    trace_id: "trace_token_refresh_uow_mismatch".to_string(),
                    sequence: 14,
                    occurred_at_ms: 1_748_486_000_001,
                    actor: actor("user_tr_uow_mismatch"),
                    workspace_id: None,
                },
            )
            .await;

        assert!(matches!(
            result,
            Err(PostgresRepositoryError::TokenRefreshPlanMismatch {
                field: "tenant_id",
                ..
            })
        ));

        let stored = grant_repo
            .get_by_id("tenant_tr_uow_mismatch", "grant_tr_uow_mismatch")
            .await?
            .expect("grant should remain after rejected plan");
        assert_eq!(stored.state, TokenGrantState::Valid);
        assert_eq!(stored.last_refresh_error.as_deref(), Some("old-error"));

        let events = PostgresAuditEventRepository::new(pool.clone())
            .find_by_tenant_and_trace_id(
                "tenant_tr_uow_mismatch",
                "trace_token_refresh_uow_mismatch",
            )
            .await?;
        assert!(events.is_empty());

        Ok(())
    });
}

#[test]
fn postgres_live_token_refresh_uow_rolls_back_when_audit_append_fails() {
    run_live_postgres_test("token_refresh_uow_rollback", |pool| async move {
        seed_user(&pool, "tenant_tr_uow_rollback", "user_tr_uow_rollback").await?;
        seed_identity(&pool, "tenant_tr_uow_rollback", "identity_tr_uow_rollback").await?;

        let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
        grant_repo
            .upsert_encrypted_grant(&encrypted_token_grant_record(
                "tenant_tr_uow_rollback",
                "grant_tr_uow_rollback",
                "identity_tr_uow_rollback",
                TokenGrantState::NeedsRefresh,
                "fp-uow-rollback-old",
            ))
            .await?;

        let audit = PostgresAuditEventRepository::new(pool.clone());
        let duplicate_event = AuditEvent::execution_succeeded(
            AuditEventContext {
                event_id: "trace_token_refresh_uow_rollback-evt-100".to_string(),
                trace_id: "trace_token_refresh_uow_rollback".to_string(),
                sequence: 100,
                occurred_at_ms: 1_748_499_999_000,
                subject: AuditSubject {
                    actor: actor("user_tr_uow_rollback"),
                    scope: scope("tenant_tr_uow_rollback"),
                    target: AuditTarget {
                        resource_type: "token_grant".to_string(),
                        resource_id: "grant_tr_uow_rollback".to_string(),
                        action_type: "token_refresh.rotate".to_string(),
                    },
                },
            },
            None,
            Some(summary("duplicate guard")),
            "noop",
        );
        audit.append(&duplicate_event, None).await?;

        let uow = PostgresTokenRefreshUnitOfWork::new(pool.clone());
        let result = uow
            .apply_planned_command_with_audit(
                planned_token_refresh_command(TokenRefreshRepositoryCommand::RotateGrantCas {
                    grant_id: TokenGrantId("grant_tr_uow_rollback".to_string()),
                    tenant_id: TenantId("tenant_tr_uow_rollback".to_string()),
                    expected_fingerprint: "fp-uow-rollback-old".to_string(),
                    expires_at_ms: Some(1_748_500_000_000),
                    refreshed_at_ms: 1_748_490_000_000,
                    encrypted_grant_blob: EncryptedGrantBlob(vec![0x44, 0x55, 0x66]),
                    grant_key_id: "key-uow-v3".to_string(),
                    new_fingerprint: "fp-uow-rollback-new".to_string(),
                }),
                TokenRefreshAuditContext {
                    trace_id: "trace_token_refresh_uow_rollback".to_string(),
                    sequence: 100,
                    occurred_at_ms: 1_748_490_000_001,
                    actor: AuditActor {
                        kind: AuditActorKind::Service,
                        actor_id: "svc_token_refresher".to_string(),
                        display_name: Some("Token Refresher".to_string()),
                    },
                    workspace_id: None,
                },
            )
            .await;
        assert!(
            result.is_err(),
            "duplicate audit event id should roll back grant mutation"
        );

        let stored = grant_repo
            .get_by_id("tenant_tr_uow_rollback", "grant_tr_uow_rollback")
            .await?
            .expect("grant should still exist after rollback");
        assert_eq!(stored.oauth_grant_fingerprint, "fp-uow-rollback-old");
        assert_eq!(stored.oauth_grant_key_id, "key-v1");
        assert_eq!(stored.encrypted_oauth_grant, vec![0x01, 0x02, 0x03]);
        assert_eq!(stored.state, TokenGrantState::NeedsRefresh);

        Ok(())
    });
}

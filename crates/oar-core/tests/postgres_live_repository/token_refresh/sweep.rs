use super::*;

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

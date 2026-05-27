use super::*;

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
fn postgres_live_token_refresh_orchestrator_with_feishu_auth_fixture_rotates_successfully() {
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
                FeishuAuthRefreshAdapter::new(client.clone()),
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
fn postgres_live_token_refresh_orchestrator_with_feishu_auth_reauth_marks_reauth_required() {
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
                FeishuAuthRefreshAdapter::new(client.clone()),
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
fn postgres_live_token_refresh_orchestrator_with_feishu_auth_plaintext_fixture_is_safe_transient() {
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
                FeishuAuthRefreshAdapter::new(client.clone()),
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

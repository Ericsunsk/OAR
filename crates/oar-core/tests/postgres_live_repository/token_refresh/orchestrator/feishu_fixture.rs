use super::*;

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

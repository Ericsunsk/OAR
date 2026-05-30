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

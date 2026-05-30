use super::*;

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

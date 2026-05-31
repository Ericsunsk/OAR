use super::harness::*;

#[test]
fn postgres_live_operational_recovery_report_lists_safe_recovery_items() {
    run_live_postgres_test("operational_recovery_report", |pool| async move {
        seed_user(&pool, "tenant_ops_recovery", "user_ops_recovery").await?;
        seed_identity(&pool, "tenant_ops_recovery", "identity_ops_recovery").await?;
        seed_user(
            &pool,
            "tenant_ops_recovery_other",
            "user_ops_recovery_other",
        )
        .await?;

        let audit = PostgresAuditEventRepository::new(pool.clone());
        let safe_outbox_id = audit
            .enqueue_outbox(
                "tenant_ops_recovery",
                "audit-events",
                "trace_safe_failed",
                &json!({
                    "trace_id": "trace_safe_failed",
                    "kind": "audit_delivery",
                    "sequence": 7
                }),
                1_000,
            )
            .await?;
        assert!(
            audit
                .mark_outbox_failed("tenant_ops_recovery", safe_outbox_id)
                .await?
        );

        sqlx::query(
            r#"
            INSERT INTO audit_outbox (
                tenant_id,
                stream,
                aggregate_id,
                payload,
                status,
                attempt_count,
                next_attempt_at
            )
            VALUES ($1, $2, $3, $4, 'failed', 3, NULL)
            "#,
        )
        .bind("tenant_ops_recovery")
        .bind("audit-events")
        .bind("trace_unsafe_failed")
        .bind(json!({ "trace_id": "refresh_token should stay hidden" }))
        .execute(&pool)
        .await?;

        let other_outbox_id = audit
            .enqueue_outbox(
                "tenant_ops_recovery_other",
                "audit-events",
                "trace_other",
                &json!({ "trace_id": "trace_other" }),
                1_000,
            )
            .await?;
        assert!(
            audit
                .mark_outbox_failed("tenant_ops_recovery_other", other_outbox_id)
                .await?
        );

        let grants = PostgresTokenGrantRepository::new(pool.clone());
        grants
            .upsert_encrypted_grant(&encrypted_token_grant_record(
                "tenant_ops_recovery",
                "grant_ops_config",
                "identity_ops_recovery",
                TokenGrantState::NeedsRefresh,
                "fp-config",
            ))
            .await?;
        grants
            .mark_refresh_failed(
                "tenant_ops_recovery",
                "grant_ops_config",
                "fp-config",
                1_748_260_500_000,
                "refresh_config_required",
            )
            .await?
            .expect("config-required grant should update");

        grants
            .upsert_encrypted_grant(&encrypted_token_grant_record(
                "tenant_ops_recovery",
                "grant_ops_reauth",
                "identity_ops_recovery",
                TokenGrantState::NeedsRefresh,
                "fp-reauth",
            ))
            .await?;
        grants
            .mark_reauth_required(
                "tenant_ops_recovery",
                "grant_ops_reauth",
                "fp-reauth",
                1_748_260_600_000,
                "invalid_grant",
            )
            .await?
            .expect("reauth grant should update");
        sqlx::query(
            r#"
            UPDATE token_grants
            SET last_refresh_error = $3
            WHERE tenant_id = $1
              AND id = $2
            "#,
        )
        .bind("tenant_ops_recovery")
        .bind("grant_ops_reauth")
        .bind("refresh_token legacy leak")
        .execute(&pool)
        .await?;

        grants
            .upsert_encrypted_grant(&encrypted_token_grant_record(
                "tenant_ops_recovery",
                "grant_ops_ok",
                "identity_ops_recovery",
                TokenGrantState::Valid,
                "fp-ok",
            ))
            .await?;
        let mut revoked_with_stale_error = encrypted_token_grant_record(
            "tenant_ops_recovery",
            "grant_ops_revoked",
            "identity_ops_recovery",
            TokenGrantState::Revoked,
            "fp-revoked",
        );
        revoked_with_stale_error.revoked_at_ms = Some(1_748_260_700_000);
        revoked_with_stale_error.last_refresh_error = Some("refresh_config_required".to_string());
        grants
            .upsert_encrypted_grant(&revoked_with_stale_error)
            .await?;

        let report = PostgresOperationalRecoveryRepository::new(pool.clone())
            .load_tenant_recovery_report("tenant_ops_recovery", 10)
            .await?;

        assert!(report.has_recovery_items());
        assert_eq!(report.tenant_id, "tenant_ops_recovery");
        assert_eq!(report.failed_audit_outbox.len(), 2);
        assert_eq!(
            report.failed_audit_outbox[0].aggregate_id,
            "trace_safe_failed"
        );
        assert!(report.failed_audit_outbox[0].payload_safe);
        assert_eq!(
            report.failed_audit_outbox[0].recommended_action,
            OperationalRecoveryAction::InspectFailedAuditOutbox
        );
        assert_eq!(
            report.failed_audit_outbox[1].aggregate_id,
            "trace_unsafe_failed"
        );
        assert!(!report.failed_audit_outbox[1].payload_safe);
        assert_eq!(report.failed_audit_outbox[1].payload, None);

        assert_eq!(report.parked_token_grants.len(), 2);
        assert!(!report
            .parked_token_grants
            .iter()
            .any(|item| item.grant_id == "grant_ops_revoked"));
        let config = report
            .parked_token_grants
            .iter()
            .find(|item| item.grant_id == "grant_ops_config")
            .expect("config-required grant should be reported");
        assert_eq!(config.state, TokenGrantState::NeedsRefresh);
        assert_eq!(
            config.safe_error.as_deref(),
            Some("refresh_config_required")
        );
        assert_eq!(
            config.recommended_action,
            OperationalRecoveryAction::FixFeishuRefreshConfigThenResume
        );

        let reauth = report
            .parked_token_grants
            .iter()
            .find(|item| item.grant_id == "grant_ops_reauth")
            .expect("reauth grant should be reported");
        assert_eq!(reauth.state, TokenGrantState::ReauthRequired);
        assert_eq!(
            reauth.safe_error.as_deref(),
            Some("<redacted refresh error>")
        );
        assert_eq!(
            reauth.recommended_action,
            OperationalRecoveryAction::AskUserToReauthorize
        );

        let debug = format!("{report:?}");
        assert!(!debug.contains("refresh_token should stay hidden"));
        assert_no_auth_refresh_sensitive_payload(&debug);
        assert!(!debug.contains("fp-config"));
        assert!(!debug.contains("fp-reauth"));

        let empty = PostgresOperationalRecoveryRepository::new(pool)
            .load_tenant_recovery_report("tenant_ops_recovery", 0)
            .await?;
        assert!(!empty.has_recovery_items());

        Ok(())
    });
}

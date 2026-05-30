use super::*;

#[test]
fn postgres_live_token_refresh_recorder_rotate_success() {
    run_live_postgres_test("token_refresh_recorder_rotate_success", |pool| async move {
        seed_user(
            &pool,
            "tenant_tr_recorder_success",
            "user_tr_recorder_success",
        )
        .await?;
        seed_identity(
            &pool,
            "tenant_tr_recorder_success",
            "identity_tr_recorder_success",
        )
        .await?;

        let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
        grant_repo
            .upsert_encrypted_grant(&encrypted_token_grant_record(
                "tenant_tr_recorder_success",
                "grant_tr_recorder_success",
                "identity_tr_recorder_success",
                TokenGrantState::NeedsRefresh,
                "fp-recorder-old",
            ))
            .await?;

        let recorder = PostgresTokenRefreshRecorder::new(pool.clone());
        let report = recorder
            .apply_planned_command_with_audit(
                planned_token_refresh_command(TokenRefreshRepositoryCommand::RotateGrantCas {
                    grant_id: TokenGrantId("grant_tr_recorder_success".to_string()),
                    tenant_id: TenantId("tenant_tr_recorder_success".to_string()),
                    expected_fingerprint: "fp-recorder-old".to_string(),
                    expires_at_ms: Some(1_748_480_000_000),
                    refreshed_at_ms: 1_748_470_000_000,
                    encrypted_grant_blob: EncryptedGrantBlob(vec![0x11, 0x22, 0x33]),
                    grant_key_id: "key-recorder-v2".to_string(),
                    new_fingerprint: "fp-recorder-new".to_string(),
                }),
                TokenRefreshAuditContext {
                    trace_id: "trace_token_refresh_recorder_success".to_string(),
                    sequence: 11,
                    occurred_at_ms: 1_748_470_000_001,
                    actor: actor("user_tr_recorder_success"),
                    workspace_id: None,
                },
            )
            .await?;

        let apply_result = report.apply_result.expect("rotate should apply");
        assert_eq!(apply_result.grant_id.0, "grant_tr_recorder_success");
        assert_eq!(apply_result.tenant_id.0, "tenant_tr_recorder_success");
        assert_eq!(apply_result.state, TokenGrantState::Valid);
        assert_eq!(report.event.target.action_type, "token_refresh.rotate");

        let stored = grant_repo
            .get_by_id("tenant_tr_recorder_success", "grant_tr_recorder_success")
            .await?
            .expect("grant should exist");
        assert_eq!(stored.oauth_grant_fingerprint, "fp-recorder-new");
        assert_eq!(stored.state, TokenGrantState::Valid);

        let events = PostgresAuditEventRepository::new(pool.clone())
            .find_by_tenant_and_trace_id(
                "tenant_tr_recorder",
                "trace_token_refresh_recorder_success",
            )
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
fn postgres_live_token_refresh_recorder_stale_fingerprint_conflict_noop() {
    run_live_postgres_test(
        "token_refresh_recorder_stale_fingerprint",
        |pool| async move {
            seed_user(&pool, "tenant_tr_recorder_noop", "user_tr_recorder_noop").await?;
            seed_identity(
                &pool,
                "tenant_tr_recorder_noop",
                "identity_tr_recorder_noop",
            )
            .await?;

            let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
            grant_repo
                .upsert_encrypted_grant(&encrypted_token_grant_record(
                    "tenant_tr_recorder_noop",
                    "grant_tr_recorder_noop",
                    "identity_tr_recorder_noop",
                    TokenGrantState::NeedsRefresh,
                    "fp-current",
                ))
                .await?;

            let recorder = PostgresTokenRefreshRecorder::new(pool.clone());
            let report = recorder
                .apply_planned_command_with_audit(
                    planned_token_refresh_command(TokenRefreshRepositoryCommand::RotateGrantCas {
                        grant_id: TokenGrantId("grant_tr_recorder_noop".to_string()),
                        tenant_id: TenantId("tenant_tr_recorder_noop".to_string()),
                        expected_fingerprint: "fp-stale".to_string(),
                        expires_at_ms: Some(1_748_490_000_000),
                        refreshed_at_ms: 1_748_480_000_000,
                        encrypted_grant_blob: EncryptedGrantBlob(vec![9, 9, 9]),
                        grant_key_id: "key-recorder-v2".to_string(),
                        new_fingerprint: "fp-noop-new".to_string(),
                    }),
                    TokenRefreshAuditContext {
                        trace_id: "trace_token_refresh_recorder_noop".to_string(),
                        sequence: 12,
                        occurred_at_ms: 1_748_480_000_001,
                        actor: actor("user_tr_recorder_noop"),
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
                .get_by_id("tenant_tr_recorder_noop", "grant_tr_recorder_noop")
                .await?
                .expect("grant should remain");
            assert_eq!(stored.oauth_grant_fingerprint, "fp-current");
            assert_eq!(stored.oauth_grant_key_id, "key-v1");
            assert_eq!(stored.encrypted_oauth_grant, vec![0x01, 0x02, 0x03]);

            let events = PostgresAuditEventRepository::new(pool.clone())
                .find_by_tenant_and_trace_id(
                    "tenant_tr_recorder_noop",
                    "trace_token_refresh_recorder_noop",
                )
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
        },
    );
}

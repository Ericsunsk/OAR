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

#[test]
fn postgres_live_token_refresh_recorder_mark_needs_refresh_redacts_audit_error() {
    run_live_postgres_test(
        "token_refresh_recorder_mark_needs_redacts",
        |pool| async move {
            seed_user(
                &pool,
                "tenant_tr_recorder_needs_redact",
                "user_tr_recorder_needs_redact",
            )
            .await?;
            seed_identity(
                &pool,
                "tenant_tr_recorder_needs_redact",
                "identity_tr_recorder_needs_redact",
            )
            .await?;

            let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
            grant_repo
                .upsert_encrypted_grant(&encrypted_token_grant_record(
                    "tenant_tr_recorder_needs_redact",
                    "grant_tr_recorder_needs_redact",
                    "identity_tr_recorder_needs_redact",
                    TokenGrantState::Valid,
                    "fp-recorder-needs-redact",
                ))
                .await?;

            let report = PostgresTokenRefreshRecorder::new(pool.clone())
                .apply_planned_command_with_audit(
                    planned_token_refresh_command(
                        TokenRefreshRepositoryCommand::MarkNeedsRefresh {
                            grant_id: TokenGrantId("grant_tr_recorder_needs_redact".to_string()),
                            tenant_id: TenantId("tenant_tr_recorder_needs_redact".to_string()),
                            expected_fingerprint: "fp-recorder-needs-redact".to_string(),
                            refreshed_at_ms: 1_748_485_000_000,
                            safe_error: "refresh_token=rt_fake Authorization: Bearer at_fake"
                                .to_string(),
                        },
                    ),
                    TokenRefreshAuditContext {
                        trace_id: "trace_token_refresh_recorder_needs_redact".to_string(),
                        sequence: 13,
                        occurred_at_ms: 1_748_485_000_001,
                        actor: actor("user_tr_recorder_needs_redact"),
                        workspace_id: None,
                    },
                )
                .await?;

            let updated = grant_repo
                .get_by_id(
                    "tenant_tr_recorder_needs_redact",
                    "grant_tr_recorder_needs_redact",
                )
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
        },
    );
}

#[test]
fn postgres_live_token_refresh_recorder_rejects_mismatched_plan_without_mutation() {
    run_live_postgres_test("token_refresh_recorder_plan_mismatch", |pool| async move {
        seed_user(
            &pool,
            "tenant_tr_recorder_mismatch",
            "user_tr_recorder_mismatch",
        )
        .await?;
        seed_identity(
            &pool,
            "tenant_tr_recorder_mismatch",
            "identity_tr_recorder_mismatch",
        )
        .await?;

        let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
        grant_repo
            .upsert_encrypted_grant(&encrypted_token_grant_record(
                "tenant_tr_recorder_mismatch",
                "grant_tr_recorder_mismatch",
                "identity_tr_recorder_mismatch",
                TokenGrantState::Valid,
                "fp-recorder-mismatch",
            ))
            .await?;

        let mut planned =
            planned_token_refresh_command(TokenRefreshRepositoryCommand::MarkNeedsRefresh {
                grant_id: TokenGrantId("grant_tr_recorder_mismatch".to_string()),
                tenant_id: TenantId("tenant_tr_recorder_mismatch".to_string()),
                expected_fingerprint: "fp-recorder-mismatch".to_string(),
                refreshed_at_ms: 1_748_486_000_000,
                safe_error: "temporarily unavailable".to_string(),
            });
        planned.report.tenant_id = TenantId("tenant_tr_recorder_other".to_string());

        let result = PostgresTokenRefreshRecorder::new(pool.clone())
            .apply_planned_command_with_audit(
                planned,
                TokenRefreshAuditContext {
                    trace_id: "trace_token_refresh_recorder_mismatch".to_string(),
                    sequence: 14,
                    occurred_at_ms: 1_748_486_000_001,
                    actor: actor("user_tr_recorder_mismatch"),
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
            .get_by_id("tenant_tr_recorder_mismatch", "grant_tr_recorder_mismatch")
            .await?
            .expect("grant should remain after rejected plan");
        assert_eq!(stored.state, TokenGrantState::Valid);
        assert_eq!(stored.last_refresh_error.as_deref(), Some("old-error"));

        let events = PostgresAuditEventRepository::new(pool.clone())
            .find_by_tenant_and_trace_id(
                "tenant_tr_recorder_mismatch",
                "trace_token_refresh_recorder_mismatch",
            )
            .await?;
        assert!(events.is_empty());

        Ok(())
    });
}

#[test]
fn postgres_live_token_refresh_recorder_rolls_back_when_audit_append_fails() {
    run_live_postgres_test("token_refresh_recorder_rollback", |pool| async move {
        seed_user(
            &pool,
            "tenant_tr_recorder_rollback",
            "user_tr_recorder_rollback",
        )
        .await?;
        seed_identity(
            &pool,
            "tenant_tr_recorder_rollback",
            "identity_tr_recorder_rollback",
        )
        .await?;

        let grant_repo = PostgresTokenGrantRepository::new(pool.clone());
        grant_repo
            .upsert_encrypted_grant(&encrypted_token_grant_record(
                "tenant_tr_recorder_rollback",
                "grant_tr_recorder_rollback",
                "identity_tr_recorder_rollback",
                TokenGrantState::NeedsRefresh,
                "fp-recorder-rollback-old",
            ))
            .await?;

        let audit = PostgresAuditEventRepository::new(pool.clone());
        let duplicate_event = AuditEvent::execution_succeeded(
            AuditEventContext {
                event_id: "trace_token_refresh_recorder_rollback-evt-100".to_string(),
                trace_id: "trace_token_refresh_recorder_rollback".to_string(),
                sequence: 100,
                occurred_at_ms: 1_748_499_999_000,
                subject: AuditSubject {
                    actor: actor("user_tr_recorder_rollback"),
                    scope: scope("tenant_tr_recorder_rollback"),
                    target: AuditTarget {
                        resource_type: "token_grant".to_string(),
                        resource_id: "grant_tr_recorder_rollback".to_string(),
                        action_type: "token_refresh.rotate".to_string(),
                    },
                },
            },
            None,
            Some(summary("duplicate guard")),
            "noop",
        );
        audit.append(&duplicate_event, None).await?;

        let recorder = PostgresTokenRefreshRecorder::new(pool.clone());
        let result = recorder
            .apply_planned_command_with_audit(
                planned_token_refresh_command(TokenRefreshRepositoryCommand::RotateGrantCas {
                    grant_id: TokenGrantId("grant_tr_recorder_rollback".to_string()),
                    tenant_id: TenantId("tenant_tr_recorder_rollback".to_string()),
                    expected_fingerprint: "fp-recorder-rollback-old".to_string(),
                    expires_at_ms: Some(1_748_500_000_000),
                    refreshed_at_ms: 1_748_490_000_000,
                    encrypted_grant_blob: EncryptedGrantBlob(vec![0x44, 0x55, 0x66]),
                    grant_key_id: "key-recorder-v3".to_string(),
                    new_fingerprint: "fp-recorder-rollback-new".to_string(),
                }),
                TokenRefreshAuditContext {
                    trace_id: "trace_token_refresh_recorder_rollback".to_string(),
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
            .get_by_id("tenant_tr_recorder_rollback", "grant_tr_recorder_rollback")
            .await?
            .expect("grant should still exist after rollback");
        assert_eq!(stored.oauth_grant_fingerprint, "fp-recorder-rollback-old");
        assert_eq!(stored.oauth_grant_key_id, "key-v1");
        assert_eq!(stored.encrypted_oauth_grant, vec![0x01, 0x02, 0x03]);
        assert_eq!(stored.state, TokenGrantState::NeedsRefresh);

        Ok(())
    });
}

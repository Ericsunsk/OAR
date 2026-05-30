use super::*;

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

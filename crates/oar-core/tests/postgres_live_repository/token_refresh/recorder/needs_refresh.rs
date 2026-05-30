use super::*;

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

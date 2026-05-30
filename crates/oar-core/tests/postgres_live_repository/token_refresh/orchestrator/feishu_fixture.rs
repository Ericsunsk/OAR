use super::feishu_fixture_support::*;
use super::*;

#[test]
fn postgres_live_token_refresh_orchestrator_with_feishu_auth_fixture_rotates_successfully() {
    run_live_postgres_test(
        "token_refresh_orchestrator_lark_fixture_rotate",
        |pool| async move {
            let tenant_id = "tenant_tr_orch_lark_fixture_rotate";
            let user_id = "user_tr_orch_lark_fixture_rotate";
            let identity_id = "identity_tr_orch_lark_fixture_rotate";
            let grant_id = "grant_tr_orch_lark_fixture_rotate";
            let grant_repo = seed_feishu_refresh_grant(
                &pool,
                tenant_id,
                user_id,
                identity_id,
                grant_id,
                TokenGrantState::NeedsRefresh,
            )
            .await?;
            let (client, mut orchestrator) =
                feishu_fixture_orchestrator(pool.clone(), AUTH_REFRESH_ROTATED_ENCRYPTED_JSON);

            let report = orchestrator
                .refresh_grant_with_audit(
                    feishu_refresh_snapshot(tenant_id, grant_id, TokenGrantState::NeedsRefresh),
                    system_time_ms(1_779_465_600_000),
                    feishu_refresh_audit_context(
                        "trace_token_refresh_orch_lark_fixture_rotate",
                        31,
                        1_779_465_600_111,
                        user_id,
                    ),
                )
                .await?;

            assert_eq!(
                report.service_report.status,
                TokenRefreshReportStatus::Succeeded
            );
            assert_eq!(report.event.target.action_type, "token_refresh.rotate");
            assert_eq!(client.calls(), 1);

            let stored = grant_repo
                .get_by_id(tenant_id, grant_id)
                .await?
                .expect("grant should exist");
            assert_eq!(stored.state, TokenGrantState::Valid);
            assert_eq!(stored.oauth_grant_fingerprint, "fp_rotated_v2");
            assert_eq!(stored.oauth_grant_key_id, "kms-key-2026-05");
            assert_ne!(stored.encrypted_oauth_grant, vec![0x01, 0x02, 0x03]);

            let payload_text = audit_refresh_payload_text(&pool, &report.event.event_id).await?;
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
            let tenant_id = "tenant_tr_orch_lark_fixture_reauth";
            let user_id = "user_tr_orch_lark_fixture_reauth";
            let identity_id = "identity_tr_orch_lark_fixture_reauth";
            let grant_id = "grant_tr_orch_lark_fixture_reauth";
            let grant_repo = seed_feishu_refresh_grant(
                &pool,
                tenant_id,
                user_id,
                identity_id,
                grant_id,
                TokenGrantState::Valid,
            )
            .await?;
            let (client, mut orchestrator) =
                feishu_fixture_orchestrator(pool.clone(), AUTH_REFRESH_REAUTH_REQUIRED_JSON);

            let report = orchestrator
                .refresh_grant_with_audit(
                    feishu_refresh_snapshot(tenant_id, grant_id, TokenGrantState::Valid),
                    system_time_ms(1_779_465_700_000),
                    feishu_refresh_audit_context(
                        "trace_token_refresh_orch_lark_fixture_reauth",
                        32,
                        1_779_465_700_111,
                        user_id,
                    ),
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
                .get_by_id(tenant_id, grant_id)
                .await?
                .expect("grant should exist");
            assert_eq!(stored.state, TokenGrantState::ReauthRequired);
            assert_eq!(stored.last_refresh_error.as_deref(), Some("invalid_grant"));

            let payload_text = audit_refresh_payload_text(&pool, &report.event.event_id).await?;
            assert_no_auth_refresh_sensitive_payload(&payload_text);

            Ok(())
        },
    );
}

#[test]
fn postgres_live_token_refresh_orchestrator_with_feishu_auth_plaintext_fixture_is_safe_transient() {
    run_live_postgres_test(
        "token_refresh_orchestrator_lark_fixture_plaintext",
        |pool| async move {
            let tenant_id = "tenant_tr_orch_lark_fixture_plaintext";
            let user_id = "user_tr_orch_lark_fixture_plaintext";
            let identity_id = "identity_tr_orch_lark_fixture_plaintext";
            let grant_id = "grant_tr_orch_lark_fixture_plaintext";
            let grant_repo = seed_feishu_refresh_grant(
                &pool,
                tenant_id,
                user_id,
                identity_id,
                grant_id,
                TokenGrantState::Valid,
            )
            .await?;
            let (client, mut orchestrator) =
                feishu_fixture_orchestrator(pool.clone(), AUTH_REFRESH_PLAINTEXT_TOKEN_LEAK_JSON);

            let report = orchestrator
                .refresh_grant_with_audit(
                    feishu_refresh_snapshot(tenant_id, grant_id, TokenGrantState::Valid),
                    system_time_ms(1_779_465_800_000),
                    feishu_refresh_audit_context(
                        "trace_token_refresh_orch_lark_fixture_plaintext",
                        33,
                        1_779_465_800_111,
                        user_id,
                    ),
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
                .get_by_id(tenant_id, grant_id)
                .await?
                .expect("grant should exist");
            assert_eq!(stored.state, TokenGrantState::NeedsRefresh);
            assert_eq!(
                stored.last_refresh_error.as_deref(),
                Some("temporarily unavailable")
            );

            let payload_text = audit_refresh_payload_text(&pool, &report.event.event_id).await?;
            assert_no_auth_refresh_sensitive_payload(&payload_text);
            assert!(!payload_text.contains("tok_access_live_should_never_parse"));
            assert!(!payload_text.contains("tok_refresh_live_should_never_parse"));
            assert!(!payload_text.contains("refresh_token="));
            assert!(!payload_text.contains("access_token="));

            Ok(())
        },
    );
}

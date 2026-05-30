use std::time::{Duration, UNIX_EPOCH};

use oar_core::action::audit_event::{AuditEventType, ExecutionStatus};
use oar_core::domain::token_refresh::types::TokenRefreshReportStatus;
use oar_core::storage::postgres::{
    PostgresAuditEventRepository, PostgresTokenGrantRepository, PostgresTokenRefreshOrchestrator,
};
use oar_lark_adapter::{FeishuOpenApiConfig, HttpResponse};

use super::harness::{
    assert_feishu_refresh_headers, assert_no_byte_secret, assert_no_sensitive_text, audit_context,
    encrypted_blob_from_plaintext, run_live_postgres_test, seed_identity_graph,
    seed_refresh_candidate_grant, success_body, RecordingAsyncHttpClient, ACTOR_ID, GRANT_ID,
    KEY_ID, NEW_REFRESH_TOKEN, OLD_FP, SEED_REFRESH_TOKEN, TENANT_ID, TRACE_ID,
};

#[test]
fn postgres_live_feishu_adapter_success_rotates_grant_and_appends_audit() {
    run_live_postgres_test("adapter_success_rotate_audit", |pool| async move {
        let key = [7; 32];
        seed_identity_graph(&pool).await?;
        let refresh_started_at_ms = current_time_ms();

        let initial_blob = encrypted_blob_from_plaintext(
            key,
            1_779_465_000_000,
            super::harness::SEED_ACCESS_TOKEN,
            SEED_REFRESH_TOKEN,
        );
        seed_refresh_candidate_grant(&pool, GRANT_ID, initial_blob.clone()).await?;

        let http_client =
            RecordingAsyncHttpClient::from_response(HttpResponse::new(200, success_body()));
        let http_probe = http_client.clone();
        let adapter = oar_lark_adapter::build_postgres_feishu_auth_refresh_adapter_with_http(
            pool.clone(),
            FeishuOpenApiConfig::default(),
            "cli_test",
            oar_lark_adapter::SecretString::new(super::harness::CLIENT_SECRET),
            KEY_ID,
            key,
            http_client,
        )?;
        let mut orchestrator = PostgresTokenRefreshOrchestrator::new(pool.clone(), adapter);
        let snapshot = PostgresTokenGrantRepository::new(pool.clone())
            .list_refresh_candidate_snapshots(
                TENANT_ID,
                UNIX_EPOCH + Duration::from_millis(1_779_466_500_000),
                1,
            )
            .await?
            .pop()
            .expect("seeded grant should be due");

        let report = orchestrator
            .refresh_grant_with_audit(
                snapshot,
                UNIX_EPOCH + Duration::from_millis(1_779_466_000_000),
                audit_context(TRACE_ID, 7),
            )
            .await?;

        assert_eq!(
            report.service_report.status,
            TokenRefreshReportStatus::Succeeded
        );
        assert!(report.service_report.adapter_called);
        assert!(report.service_report.sink_called);
        assert_eq!(report.event.event_type, AuditEventType::ExecutionSucceeded);
        assert_eq!(report.event.target.resource_type, "token_grant");
        assert_eq!(report.event.target.resource_id, GRANT_ID);
        assert_eq!(report.event.target.action_type, "token_refresh.rotate");
        assert_eq!(
            report
                .event
                .execution
                .as_ref()
                .map(|execution| &execution.status),
            Some(&ExecutionStatus::Succeeded)
        );
        assert_eq!(report.event.actor.actor_id, ACTOR_ID);

        let rotated = PostgresTokenGrantRepository::new(pool.clone())
            .get_by_id(TENANT_ID, GRANT_ID)
            .await?
            .expect("grant should still exist");
        assert_eq!(
            rotated.state,
            oar_core::domain::identity::TokenGrantState::Valid
        );
        assert_eq!(rotated.oauth_grant_key_id, KEY_ID);
        assert_ne!(rotated.oauth_grant_fingerprint, OLD_FP);
        assert_ne!(rotated.encrypted_oauth_grant, initial_blob);
        assert_eq!(rotated.last_refresh_error, None);
        let refreshed_at_ms = rotated
            .refreshed_at_ms
            .expect("production builder should set refreshed_at_ms from system clock");
        assert!(refreshed_at_ms >= refresh_started_at_ms);
        assert!(refreshed_at_ms <= current_time_ms());
        assert_eq!(
            rotated.expires_at_ms,
            Some(refreshed_at_ms.saturating_add(7_200_000))
        );
        assert_no_byte_secret(&rotated.encrypted_oauth_grant);

        let audit_events = PostgresAuditEventRepository::new(pool.clone())
            .find_by_tenant_and_trace_id(TENANT_ID, TRACE_ID)
            .await?;
        assert_eq!(audit_events, vec![report.event.clone()]);
        let audit_text = serde_json::to_string(&audit_events)?;
        assert_no_sensitive_text(&audit_text);

        let sent_requests = http_probe.requests();
        assert_eq!(sent_requests.len(), 1);
        let sent = &sent_requests[0];
        assert_eq!(
            sent.url,
            "https://open.feishu.cn/open-apis/authen/v2/oauth/token"
        );
        assert_eq!(sent.method, "POST");
        assert_feishu_refresh_headers(&sent.headers);
        assert_eq!(sent.max_response_bytes, 64 * 1024);
        assert_eq!(sent.body["grant_type"], "refresh_token");
        assert_eq!(sent.body["client_id"], "cli_test");
        assert_eq!(sent.body["client_secret"], super::harness::CLIENT_SECRET);
        assert_eq!(sent.body["refresh_token"], SEED_REFRESH_TOKEN);
        assert_eq!(
            sent.body["scope"],
            "offline_access auth:user.id:read okr.progress.write"
        );
        assert_ne!(sent.body["refresh_token"], NEW_REFRESH_TOKEN);
        assert_no_sensitive_text(&format!("{sent:?}"));
        assert_no_sensitive_text(&format!("{report:?}"));

        Ok(())
    });
}

fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_millis() as u64
}

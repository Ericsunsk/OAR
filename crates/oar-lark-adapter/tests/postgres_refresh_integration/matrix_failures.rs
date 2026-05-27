use std::time::{Duration, UNIX_EPOCH};

use oar_core::action::audit_event::{AuditEventType, ExecutionStatus};
use oar_core::domain::identity::TokenGrantState;
use oar_core::domain::token_refresh::types::{TokenRefreshCommandKind, TokenRefreshReportStatus};
use oar_core::storage::postgres::{
    PostgresAuditEventRepository, PostgresTokenGrantRepository, PostgresTokenRefreshOrchestrator,
};
use oar_lark_adapter::oauth::HttpClientFailure;
use oar_lark_adapter::{AesGcmGrantEncryptor, FeishuOpenApiConfig, HttpResponse};
use sqlx::PgPool;

use super::harness::{
    assert_feishu_refresh_headers, assert_no_byte_secret, assert_no_sensitive_text, audit_context,
    encrypted_blob_from_plaintext, make_material_provider, run_live_postgres_test,
    seed_identity_graph, seed_refresh_candidate_grant, failure_body, RecordingAsyncHttpClient,
    ACTOR_ID, KEY_ID, OLD_FP, SEED_ACCESS_TOKEN, SEED_REFRESH_TOKEN, TENANT_ID, TestResult,
};

#[derive(Clone)]
struct FailureCase {
    name: &'static str,
    grant_id: &'static str,
    trace_id: &'static str,
    http_result: Result<HttpResponse, HttpClientFailure>,
    expected_state: TokenGrantState,
    expected_error: &'static str,
    expected_action_type: &'static str,
    expected_command: TokenRefreshCommandKind,
    expected_candidate_after_failure: bool,
}

#[test]
fn postgres_live_feishu_adapter_failure_matrix_updates_state_and_audit_safely() {
    let cases = [
        FailureCase {
            name: "config_required_20074",
            grant_id: "grant_adapter_pg_refresh_cfg",
            trace_id: "trace_adapter_pg_refresh_cfg",
            http_result: Ok(HttpResponse::new(400, failure_body(20074))),
            expected_state: TokenGrantState::NeedsRefresh,
            expected_error: "refresh_config_required",
            expected_action_type: "token_refresh.mark_config_required",
            expected_command: TokenRefreshCommandKind::MarkConfigRequired,
            expected_candidate_after_failure: false,
        },
        FailureCase {
            name: "reauth_required_20037",
            grant_id: "grant_adapter_pg_refresh_reauth",
            trace_id: "trace_adapter_pg_refresh_reauth",
            http_result: Ok(HttpResponse::new(400, failure_body(20037))),
            expected_state: TokenGrantState::ReauthRequired,
            expected_error: "invalid_grant",
            expected_action_type: "token_refresh.mark_reauth_required",
            expected_command: TokenRefreshCommandKind::MarkReauthRequired,
            expected_candidate_after_failure: false,
        },
        FailureCase {
            name: "transient_http_5xx",
            grant_id: "grant_adapter_pg_refresh_transient",
            trace_id: "trace_adapter_pg_refresh_transient",
            http_result: Ok(HttpResponse::new(503, "upstream unavailable")),
            expected_state: TokenGrantState::NeedsRefresh,
            expected_error: "temporarily unavailable",
            expected_action_type: "token_refresh.mark_needs_refresh",
            expected_command: TokenRefreshCommandKind::MarkNeedsRefresh,
            expected_candidate_after_failure: true,
        },
        FailureCase {
            name: "oversized_response_transport",
            grant_id: "grant_adapter_pg_refresh_oversized",
            trace_id: "trace_adapter_pg_refresh_oversized",
            http_result: Err(HttpClientFailure::OversizedResponse {
                max_response_bytes: 64 * 1024,
            }),
            expected_state: TokenGrantState::NeedsRefresh,
            expected_error: "temporarily unavailable",
            expected_action_type: "token_refresh.mark_needs_refresh",
            expected_command: TokenRefreshCommandKind::MarkNeedsRefresh,
            expected_candidate_after_failure: true,
        },
    ];

    for case in cases {
        run_live_postgres_test(
            &format!("adapter_failure_matrix_{}", case.name),
            move |pool| async move { run_failure_case(pool, case).await },
        );
    }
}

async fn run_failure_case(pool: PgPool, case: FailureCase) -> TestResult {
    let key = [7; 32];
    seed_identity_graph(&pool).await?;

    let initial_blob =
        encrypted_blob_from_plaintext(key, 1_779_465_000_000, SEED_ACCESS_TOKEN, SEED_REFRESH_TOKEN);
    seed_refresh_candidate_grant(&pool, case.grant_id, initial_blob.clone()).await?;

    let material_provider = make_material_provider(pool.clone(), key);
    let http_client = RecordingAsyncHttpClient::from_result(case.http_result.clone());
    let http_probe = http_client.clone();
    let adapter = oar_lark_adapter::build_feishu_auth_refresh_adapter(
        FeishuOpenApiConfig::default(),
        material_provider,
        AesGcmGrantEncryptor::with_clock(
            KEY_ID,
            key,
            super::harness::FixedClock {
                now_ms: 1_779_466_000_000,
            },
        ),
        http_client,
    )?;
    let mut orchestrator = PostgresTokenRefreshOrchestrator::new(pool.clone(), adapter);
    let snapshot = PostgresTokenGrantRepository::new(pool.clone())
        .list_refresh_candidate_snapshots(
            TENANT_ID,
            UNIX_EPOCH + Duration::from_millis(1_779_466_500_000),
            10,
        )
        .await?
        .into_iter()
        .find(|candidate| candidate.grant_id.0 == case.grant_id)
        .expect("seeded failure grant should be due");

    let report = orchestrator
        .refresh_grant_with_audit(
            snapshot,
            UNIX_EPOCH + Duration::from_millis(1_779_466_000_000),
            audit_context(case.trace_id, 11),
        )
        .await?;

    assert_eq!(
        report.service_report.status,
        TokenRefreshReportStatus::Succeeded
    );
    assert!(report.service_report.adapter_called);
    assert!(report.service_report.sink_called);
    assert_eq!(report.service_report.command, Some(case.expected_command));
    assert_eq!(
        report.service_report.safe_error.as_deref(),
        Some(case.expected_error)
    );
    assert_eq!(report.event.event_type, AuditEventType::ExecutionSucceeded);
    assert_eq!(report.event.target.resource_type, "token_grant");
    assert_eq!(report.event.target.resource_id, case.grant_id);
    assert_eq!(report.event.target.action_type, case.expected_action_type);
    assert_eq!(
        report
            .event
            .execution
            .as_ref()
            .map(|execution| &execution.status),
        Some(&ExecutionStatus::Succeeded)
    );
    assert_eq!(
        report.event.actor.as_ref().map(|actor| actor.actor_id.as_str()),
        Some(ACTOR_ID)
    );

    let updated = PostgresTokenGrantRepository::new(pool.clone())
        .get_by_id(TENANT_ID, case.grant_id)
        .await?
        .expect("failure grant should still exist");
    assert_eq!(updated.state, case.expected_state);
    assert_eq!(updated.oauth_grant_key_id, KEY_ID);
    assert_eq!(updated.oauth_grant_fingerprint, OLD_FP);
    assert_eq!(updated.encrypted_oauth_grant, initial_blob);
    assert_eq!(
        updated.last_refresh_error.as_deref(),
        Some(case.expected_error)
    );
    if case.expected_state == TokenGrantState::ReauthRequired {
        assert_eq!(updated.reauth_required_at_ms, Some(1_779_466_000_000));
    } else {
        assert_eq!(updated.reauth_required_at_ms, None);
        assert_eq!(updated.refreshed_at_ms, Some(1_779_466_000_000));
    }
    assert_no_byte_secret(&updated.encrypted_oauth_grant);

    let candidate_after_failure = PostgresTokenGrantRepository::new(pool.clone())
        .list_refresh_candidate_snapshots(
            TENANT_ID,
            UNIX_EPOCH + Duration::from_millis(1_779_466_500_000),
            10,
        )
        .await?
        .into_iter()
        .any(|candidate| candidate.grant_id.0 == case.grant_id);
    assert_eq!(
        candidate_after_failure,
        case.expected_candidate_after_failure
    );

    let audit_events = PostgresAuditEventRepository::new(pool.clone())
        .find_by_tenant_and_trace_id(TENANT_ID, case.trace_id)
        .await?;
    assert_eq!(audit_events, vec![report.event.clone()]);
    let audit_text = serde_json::to_string(&audit_events)?;
    assert_no_sensitive_text(&audit_text);

    let sent_requests = http_probe.requests();
    assert_eq!(sent_requests.len(), 1);
    let sent = &sent_requests[0];
    assert_eq!(sent.method, "POST");
    assert_feishu_refresh_headers(&sent.headers);
    assert_eq!(sent.max_response_bytes, 64 * 1024);
    assert_eq!(sent.body["refresh_token"], SEED_REFRESH_TOKEN);
    assert_eq!(sent.body["client_secret"], super::harness::CLIENT_SECRET);
    assert_no_sensitive_text(&format!("{sent:?}"));
    assert_no_sensitive_text(&format!("{report:?}"));

    Ok(())
}

use std::time::{Duration, UNIX_EPOCH};

use oar_core::action::audit_event::{AuditEventType, ExecutionStatus};
use oar_core::domain::identity::TokenGrantState;
use oar_core::domain::token_refresh::types::{TokenRefreshCommandKind, TokenRefreshReportStatus};
use oar_core::storage::postgres::{
    PostgresAuditEventRepository, PostgresTokenGrantRepository, PostgresTokenRefreshOrchestrator,
};
use oar_lark_adapter::{AesGcmGrantEncryptor, FeishuOpenApiConfig, HttpResponse};
use sqlx::PgPool;

use super::harness::{
    assert_no_byte_secret, assert_no_sensitive_text, audit_context, encrypted_blob_from_plaintext,
    make_material_provider, run_live_postgres_test, seed_identity_graph, seed_refresh_candidate_grant,
    success_body, RecordingAsyncHttpClient, KEY_ID, OLD_FP, SEED_ACCESS_TOKEN, SEED_REFRESH_TOKEN,
    TENANT_ID, TestResult,
};

#[derive(Clone)]
struct MaterialFailureCase {
    name: &'static str,
    grant_id: &'static str,
    trace_id: &'static str,
    mutator: MaterialFailureMutation,
    resolver_key: [u8; 32],
    expected_status: TokenRefreshReportStatus,
    expected_event_type: AuditEventType,
    expected_action_type: &'static str,
    expect_row_after_failure: bool,
}

#[derive(Clone)]
enum MaterialFailureMutation {
    Noop,
    DeleteGrantRow,
    OverwriteFingerprint(&'static str),
    OverwriteEncryptedBlob(Vec<u8>),
}

#[test]
fn postgres_live_feishu_adapter_material_failure_matrix_fails_closed_without_http_or_rotation() {
    let cases = [
        MaterialFailureCase {
            name: "missing_db_row",
            grant_id: "grant_adapter_pg_refresh_material_missing",
            trace_id: "trace_adapter_pg_refresh_material_missing",
            mutator: MaterialFailureMutation::DeleteGrantRow,
            resolver_key: [7; 32],
            expected_status: TokenRefreshReportStatus::ConflictNoop,
            expected_event_type: AuditEventType::ExecutionFailed,
            expected_action_type: "token_refresh.conflict_noop",
            expect_row_after_failure: false,
        },
        MaterialFailureCase {
            name: "fingerprint_mismatch",
            grant_id: "grant_adapter_pg_refresh_material_fp_mismatch",
            trace_id: "trace_adapter_pg_refresh_material_fp_mismatch",
            mutator: MaterialFailureMutation::OverwriteFingerprint("fp-material-mismatch"),
            resolver_key: [7; 32],
            expected_status: TokenRefreshReportStatus::ConflictNoop,
            expected_event_type: AuditEventType::ExecutionFailed,
            expected_action_type: "token_refresh.conflict_noop",
            expect_row_after_failure: true,
        },
        MaterialFailureCase {
            name: "malformed_encrypted_blob",
            grant_id: "grant_adapter_pg_refresh_material_blob_malformed",
            trace_id: "trace_adapter_pg_refresh_material_blob_malformed",
            mutator: MaterialFailureMutation::OverwriteEncryptedBlob(vec![1, 2, 3]),
            resolver_key: [7; 32],
            expected_status: TokenRefreshReportStatus::Succeeded,
            expected_event_type: AuditEventType::ExecutionSucceeded,
            expected_action_type: "token_refresh.mark_needs_refresh",
            expect_row_after_failure: true,
        },
        MaterialFailureCase {
            name: "wrong_key_decrypt_failure",
            grant_id: "grant_adapter_pg_refresh_material_wrong_key",
            trace_id: "trace_adapter_pg_refresh_material_wrong_key",
            mutator: MaterialFailureMutation::Noop,
            resolver_key: [13; 32],
            expected_status: TokenRefreshReportStatus::Succeeded,
            expected_event_type: AuditEventType::ExecutionSucceeded,
            expected_action_type: "token_refresh.mark_needs_refresh",
            expect_row_after_failure: true,
        },
    ];

    for case in cases {
        run_live_postgres_test(
            &format!("adapter_material_failure_matrix_{}", case.name),
            move |pool| async move { run_material_failure_case(pool, case).await },
        );
    }
}

async fn run_material_failure_case(pool: PgPool, case: MaterialFailureCase) -> TestResult {
    let encryption_key = [7; 32];
    seed_identity_graph(&pool).await?;

    let initial_blob = encrypted_blob_from_plaintext(
        encryption_key,
        1_779_465_000_000,
        SEED_ACCESS_TOKEN,
        SEED_REFRESH_TOKEN,
    );
    seed_refresh_candidate_grant(&pool, case.grant_id, initial_blob.clone()).await?;

    let snapshot = PostgresTokenGrantRepository::new(pool.clone())
        .list_refresh_candidate_snapshots(
            TENANT_ID,
            UNIX_EPOCH + Duration::from_millis(1_779_466_500_000),
            10,
        )
        .await?
        .into_iter()
        .find(|candidate| candidate.grant_id.0 == case.grant_id)
        .expect("seeded material failure grant should be due");

    match &case.mutator {
        MaterialFailureMutation::Noop => {}
        MaterialFailureMutation::DeleteGrantRow => {
            sqlx::query(
                r#"
                DELETE FROM token_grants
                WHERE tenant_id = $1 AND id = $2
                "#,
            )
            .bind(TENANT_ID)
            .bind(case.grant_id)
            .execute(&pool)
            .await?;
        }
        MaterialFailureMutation::OverwriteFingerprint(fingerprint) => {
            sqlx::query(
                r#"
                UPDATE token_grants
                SET oauth_grant_fingerprint = $3
                WHERE tenant_id = $1 AND id = $2
                "#,
            )
            .bind(TENANT_ID)
            .bind(case.grant_id)
            .bind(*fingerprint)
            .execute(&pool)
            .await?;
        }
        MaterialFailureMutation::OverwriteEncryptedBlob(blob) => {
            sqlx::query(
                r#"
                UPDATE token_grants
                SET encrypted_oauth_grant = $3
                WHERE tenant_id = $1 AND id = $2
                "#,
            )
            .bind(TENANT_ID)
            .bind(case.grant_id)
            .bind(blob)
            .execute(&pool)
            .await?;
        }
    }

    let material_provider = make_material_provider(pool.clone(), case.resolver_key);
    let http_client =
        RecordingAsyncHttpClient::from_response(HttpResponse::new(200, success_body()));
    let http_probe = http_client.clone();
    let adapter = oar_lark_adapter::build_feishu_auth_refresh_adapter(
        FeishuOpenApiConfig::default(),
        material_provider,
        AesGcmGrantEncryptor::with_clock(
            KEY_ID,
            encryption_key,
            super::harness::FixedClock {
                now_ms: 1_779_466_000_000,
            },
        ),
        http_client,
    )?;
    let mut orchestrator = PostgresTokenRefreshOrchestrator::new(pool.clone(), adapter);

    let report = orchestrator
        .refresh_grant_with_audit(
            snapshot,
            UNIX_EPOCH + Duration::from_millis(1_779_466_000_000),
            audit_context(case.trace_id, 13),
        )
        .await?;

    assert_eq!(report.service_report.status, case.expected_status);
    assert!(report.service_report.adapter_called);
    assert!(report.service_report.sink_called);
    assert_eq!(
        report.service_report.command,
        Some(TokenRefreshCommandKind::MarkNeedsRefresh)
    );
    assert_eq!(
        report.service_report.safe_error.as_deref(),
        Some("temporarily unavailable")
    );
    assert_eq!(report.event.event_type, case.expected_event_type);
    assert_eq!(report.event.target.resource_type, "token_grant");
    assert_eq!(report.event.target.resource_id, case.grant_id);
    assert_eq!(report.event.target.action_type, case.expected_action_type);
    let expected_execution_status = match case.expected_status {
        TokenRefreshReportStatus::Succeeded => ExecutionStatus::Succeeded,
        TokenRefreshReportStatus::ConflictNoop => ExecutionStatus::Failed,
        TokenRefreshReportStatus::ShortCircuited(_) => ExecutionStatus::Denied,
    };
    assert_eq!(
        report
            .event
            .execution
            .as_ref()
            .map(|execution| &execution.status),
        Some(&expected_execution_status)
    );

    let sent_requests = http_probe.requests();
    assert_eq!(
        sent_requests.len(),
        0,
        "material failure should not issue outbound feishu http"
    );

    if case.expect_row_after_failure {
        let updated = PostgresTokenGrantRepository::new(pool.clone())
            .get_by_id(TENANT_ID, case.grant_id)
            .await?
            .expect("material failure grant should still exist");
        assert_eq!(updated.state, TokenGrantState::NeedsRefresh);
        assert_eq!(updated.oauth_grant_key_id, KEY_ID);
        match case.expected_status {
            TokenRefreshReportStatus::Succeeded => {
                assert_eq!(updated.oauth_grant_fingerprint, OLD_FP);
                assert_eq!(updated.encrypted_oauth_grant, initial_blob);
                assert_eq!(
                    updated.last_refresh_error.as_deref(),
                    Some("temporarily unavailable")
                );
                assert_eq!(updated.refreshed_at_ms, Some(1_779_466_000_000));
            }
            TokenRefreshReportStatus::ConflictNoop => {
                match &case.mutator {
                    MaterialFailureMutation::OverwriteFingerprint(fingerprint) => {
                        assert_eq!(updated.oauth_grant_fingerprint, *fingerprint);
                    }
                    _ => assert_eq!(updated.oauth_grant_fingerprint, OLD_FP),
                }
                assert_eq!(updated.encrypted_oauth_grant, initial_blob);
                assert_eq!(updated.last_refresh_error.as_deref(), Some("old-error"));
                assert_eq!(updated.refreshed_at_ms, Some(1_779_465_000_000));
            }
            TokenRefreshReportStatus::ShortCircuited(_) => unreachable!("not used in this matrix"),
        }
        assert_eq!(updated.reauth_required_at_ms, None);
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
        assert!(candidate_after_failure);
    } else {
        let deleted = PostgresTokenGrantRepository::new(pool.clone())
            .get_by_id(TENANT_ID, case.grant_id)
            .await?;
        assert_eq!(deleted, None);
    }

    let audit_events = PostgresAuditEventRepository::new(pool.clone())
        .find_by_tenant_and_trace_id(TENANT_ID, case.trace_id)
        .await?;
    assert_eq!(audit_events, vec![report.event.clone()]);
    let audit_text = serde_json::to_string(&audit_events)?;
    assert_no_sensitive_text(&audit_text);
    assert_no_sensitive_text(&format!("{report:?}"));

    Ok(())
}

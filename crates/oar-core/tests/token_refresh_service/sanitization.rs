use std::time::{Duration, SystemTime};

use oar_core::domain::identity::{TenantId, TokenGrantId, TokenGrantState};
use oar_core::domain::token_refresh::service::{TokenRefreshService, TokenRefreshServiceError};
use oar_core::domain::token_refresh::types::{
    EncryptedGrantMaterial, RefreshOutcome, TokenRefreshDecision, TokenRefreshRepositoryCommand,
};

use crate::common::{
    sample_apply_result, sample_grant, sample_snapshot, transient_failure_outcome,
    FakeAuthRefreshAdapter, FakeCommandSink,
};

#[test]
fn encrypted_material_debug_redacts_payload() {
    let material = EncryptedGrantMaterial {
        encrypted_primary: vec![9, 9, 9],
        encrypted_renewal: vec![8, 8, 8],
    };
    let debug_output = format!("{material:?}");
    assert!(debug_output.contains("[REDACTED]"));
    assert!(!debug_output.contains("9, 9, 9"));
    assert!(!debug_output.contains("8, 8, 8"));
}

#[test]
fn decision_bridge_and_blob_debug_redact_bytes() {
    let decision = TokenRefreshDecision::RotateGrantCas {
        grant_id: TokenGrantId("grant_01".to_string()),
        tenant_id: TenantId("tenant_01".to_string()),
        expected_fingerprint: "fp_old".to_string(),
        rotated_material: EncryptedGrantMaterial {
            encrypted_primary: vec![9, 9, 9],
            encrypted_renewal: vec![8, 8, 8],
        },
        key_id: "key_v2".to_string(),
        new_fingerprint: "fp_new".to_string(),
        refreshed_at: SystemTime::UNIX_EPOCH,
        expires_at: None,
    };

    let command = decision
        .into_repository_command_at(SystemTime::UNIX_EPOCH)
        .expect("bridge command");
    let debug_output = format!("{command:?}");

    assert!(debug_output.contains("[REDACTED]"));
    assert!(!debug_output.contains("9, 9, 9"));
    assert!(!debug_output.contains("8, 8, 8"));
}

#[test]
fn service_report_and_audit_summary_do_not_leak_tokens_or_encrypted_bytes() {
    let grant = sample_grant(TokenGrantState::NeedsRefresh, Some("refresh-old"));
    let snapshot = sample_snapshot(&grant);
    let adapter = FakeAuthRefreshAdapter::new(RefreshOutcome::Success {
        rotated_material: EncryptedGrantMaterial {
            encrypted_primary: vec![9, 9, 9],
            encrypted_renewal: vec![8, 8, 8],
        },
        key_id: "key_v2".to_string(),
        new_fingerprint: "fp_new".to_string(),
        refreshed_at: SystemTime::UNIX_EPOCH + Duration::from_secs(1),
        expires_at: None,
    });
    let sink = FakeCommandSink::new(Ok(Some(sample_apply_result(
        TokenGrantState::Valid,
        "fp_new",
    ))));
    let mut service = TokenRefreshService::new(adapter, sink);

    let report = service
        .refresh_grant_at(snapshot, SystemTime::UNIX_EPOCH + Duration::from_secs(2))
        .expect("service refresh should succeed");
    let audit = report.audit_summary();
    let report_debug = format!("{report:?}");
    let audit_debug = format!("{audit:?}");

    assert!(!report_debug.contains("access-old"));
    assert!(!report_debug.contains("refresh-old"));
    assert!(!report_debug.contains("9, 9, 9"));
    assert!(!report_debug.contains("8, 8, 8"));
    assert!(!audit_debug.contains("access-old"));
    assert!(!audit_debug.contains("refresh-old"));
    assert!(!audit_debug.contains("9, 9, 9"));
    assert!(!audit_debug.contains("8, 8, 8"));
}

#[test]
fn service_report_redacts_token_like_adapter_errors() {
    let grant = sample_grant(TokenGrantState::NeedsRefresh, Some("refresh-old"));
    let snapshot = sample_snapshot(&grant);
    let adapter = FakeAuthRefreshAdapter::new(transient_failure_outcome(
        "opaque-token-fragment-without-keyword",
    ));
    let sink = FakeCommandSink::new(Ok(Some(sample_apply_result(
        TokenGrantState::NeedsRefresh,
        "fp_old",
    ))));
    let mut service = TokenRefreshService::new(adapter, sink.clone());

    let report = service
        .refresh_grant_at(snapshot, SystemTime::UNIX_EPOCH)
        .expect("service refresh should succeed");
    let audit = report.audit_summary();

    assert_eq!(
        report.safe_error.as_deref(),
        Some("<redacted refresh error>")
    );
    assert_eq!(
        audit.safe_error.as_deref(),
        Some("<redacted refresh error>")
    );
    match sink.last_command().expect("expected command") {
        TokenRefreshRepositoryCommand::MarkNeedsRefresh { safe_error, .. } => {
            assert_eq!(safe_error, "<redacted refresh error>");
        }
        other => panic!("expected MarkNeedsRefresh, got {other:?}"),
    }
    assert!(!format!("{report:?}").contains("opaque-token-fragment"));
    assert!(!format!("{audit:?}").contains("opaque-token-fragment"));
}

#[test]
fn service_error_redacts_command_sink_error_outputs() {
    let error: TokenRefreshServiceError<String> =
        TokenRefreshServiceError::CommandSink("refresh-token-secret".to_string());

    assert_eq!(error.to_string(), "token refresh command sink failed");
    assert!(format!("{error:?}").contains("[REDACTED]"));
    assert!(!format!("{error:?}").contains("refresh-token-secret"));
    assert!(std::error::Error::source(&error).is_none());
}

use std::time::{Duration, SystemTime};

use oar_core::domain::identity::TokenGrantState;
use oar_core::domain::token_refresh::service::TokenRefreshService;
use oar_core::domain::token_refresh::types::{
    EncryptedGrantMaterial, RefreshOutcome, TokenRefreshCommandKind, TokenRefreshDecisionKind,
    TokenRefreshReportStatus, TokenRefreshRepositoryCommand, TokenRefreshShortCircuitReason,
};

use crate::common::{
    sample_apply_result, sample_grant, sample_snapshot, FakeAuthRefreshAdapter, FakeCommandSink,
};

#[test]
fn service_success_path_calls_adapter_and_sink_once_and_reports_success() {
    let grant = sample_grant(TokenGrantState::NeedsRefresh, Some("refresh-old"));
    let snapshot = sample_snapshot(&grant);
    let adapter = FakeAuthRefreshAdapter::new(RefreshOutcome::Success {
        rotated_material: EncryptedGrantMaterial {
            encrypted_primary: vec![1, 2, 3],
            encrypted_renewal: vec![4, 5, 6],
        },
        key_id: "key_v2".to_string(),
        new_fingerprint: "fp_new".to_string(),
        refreshed_at: SystemTime::UNIX_EPOCH + Duration::from_secs(2),
        expires_at: None,
    });
    let sink = FakeCommandSink::new(Ok(Some(sample_apply_result(
        TokenGrantState::Valid,
        "fp_new",
    ))));
    let mut service = TokenRefreshService::new(adapter.clone(), sink.clone());

    let report = service
        .refresh_grant_at(snapshot, SystemTime::UNIX_EPOCH + Duration::from_secs(3))
        .expect("service refresh should succeed");

    assert_eq!(adapter.calls(), 1);
    assert_eq!(sink.calls(), 1);
    assert_eq!(report.status, TokenRefreshReportStatus::Succeeded);
    assert_eq!(
        report.decision,
        Some(TokenRefreshDecisionKind::RotateGrantCas)
    );
    assert_eq!(
        report.command,
        Some(TokenRefreshCommandKind::RotateGrantCas)
    );
    assert_eq!(report.safe_error, None);

    match sink.last_command().expect("expected command") {
        TokenRefreshRepositoryCommand::RotateGrantCas { .. } => {}
        other => panic!("expected RotateGrantCas, got {other:?}"),
    }
}

#[test]
fn service_transient_failure_marks_needs_refresh() {
    let grant = sample_grant(TokenGrantState::NeedsRefresh, Some("refresh-old"));
    let snapshot = sample_snapshot(&grant);
    let adapter = FakeAuthRefreshAdapter::new(RefreshOutcome::TransientFailure {
        safe_error: "temporarily unavailable".to_string(),
    });
    let sink = FakeCommandSink::new(Ok(Some(sample_apply_result(
        TokenGrantState::NeedsRefresh,
        "fp_old",
    ))));
    let mut service = TokenRefreshService::new(adapter.clone(), sink.clone());

    let report = service
        .refresh_grant_at(snapshot, SystemTime::UNIX_EPOCH + Duration::from_secs(8))
        .expect("service refresh should succeed");

    assert_eq!(adapter.calls(), 1);
    assert_eq!(sink.calls(), 1);
    assert_eq!(report.status, TokenRefreshReportStatus::Succeeded);
    assert_eq!(
        report.decision,
        Some(TokenRefreshDecisionKind::MarkNeedsRefresh)
    );
    assert_eq!(
        report.command,
        Some(TokenRefreshCommandKind::MarkNeedsRefresh)
    );
    assert_eq!(
        report.safe_error.as_deref(),
        Some("temporarily unavailable")
    );

    match sink.last_command().expect("expected command") {
        TokenRefreshRepositoryCommand::MarkNeedsRefresh { safe_error, .. } => {
            assert_eq!(safe_error, "temporarily unavailable");
        }
        other => panic!("expected MarkNeedsRefresh, got {other:?}"),
    }
}

#[test]
fn service_reauth_failure_marks_reauth_required() {
    let grant = sample_grant(TokenGrantState::Valid, Some("refresh-old"));
    let snapshot = sample_snapshot(&grant);
    let adapter = FakeAuthRefreshAdapter::new(RefreshOutcome::ReauthFailure {
        safe_error: "invalid_grant".to_string(),
    });
    let sink = FakeCommandSink::new(Ok(Some(sample_apply_result(
        TokenGrantState::ReauthRequired,
        "fp_old",
    ))));
    let mut service = TokenRefreshService::new(adapter.clone(), sink.clone());

    let report = service
        .refresh_grant_at(snapshot, SystemTime::UNIX_EPOCH + Duration::from_secs(13))
        .expect("service refresh should succeed");

    assert_eq!(adapter.calls(), 1);
    assert_eq!(sink.calls(), 1);
    assert_eq!(report.status, TokenRefreshReportStatus::Succeeded);
    assert_eq!(
        report.decision,
        Some(TokenRefreshDecisionKind::MarkReauthRequired)
    );
    assert_eq!(
        report.command,
        Some(TokenRefreshCommandKind::MarkReauthRequired)
    );
    assert_eq!(report.safe_error.as_deref(), Some("invalid_grant"));

    match sink.last_command().expect("expected command") {
        TokenRefreshRepositoryCommand::MarkReauthRequired { safe_error, .. } => {
            assert_eq!(safe_error, "invalid_grant");
        }
        other => panic!("expected MarkReauthRequired, got {other:?}"),
    }
}

#[test]
fn service_config_required_marks_config_required() {
    let grant = sample_grant(TokenGrantState::Valid, Some("refresh-old"));
    let snapshot = sample_snapshot(&grant);
    let adapter = FakeAuthRefreshAdapter::new(RefreshOutcome::ConfigRequired {
        safe_error: "refresh_config_required".to_string(),
    });
    let sink = FakeCommandSink::new(Ok(Some(sample_apply_result(
        TokenGrantState::NeedsRefresh,
        "fp_old",
    ))));
    let mut service = TokenRefreshService::new(adapter.clone(), sink.clone());

    let report = service
        .refresh_grant_at(snapshot, SystemTime::UNIX_EPOCH + Duration::from_secs(17))
        .expect("service refresh should succeed");

    assert_eq!(adapter.calls(), 1);
    assert_eq!(sink.calls(), 1);
    assert_eq!(report.status, TokenRefreshReportStatus::Succeeded);
    assert_eq!(
        report.decision,
        Some(TokenRefreshDecisionKind::MarkConfigRequired)
    );
    assert_eq!(
        report.command,
        Some(TokenRefreshCommandKind::MarkConfigRequired)
    );
    assert_eq!(
        report.safe_error.as_deref(),
        Some("refresh_config_required")
    );

    match sink.last_command().expect("expected command") {
        TokenRefreshRepositoryCommand::MarkConfigRequired { safe_error, .. } => {
            assert_eq!(safe_error, "refresh_config_required");
        }
        other => panic!("expected MarkConfigRequired, got {other:?}"),
    }
}

#[test]
fn service_short_circuits_revoked_reauth_and_missing_refresh_material() {
    let cases = [
        (
            sample_grant(TokenGrantState::Revoked, Some("refresh-old")),
            TokenRefreshShortCircuitReason::Revoked,
        ),
        (
            sample_grant(TokenGrantState::ReauthRequired, Some("refresh-old")),
            TokenRefreshShortCircuitReason::ReauthRequired,
        ),
        (
            sample_grant(TokenGrantState::NeedsRefresh, None),
            TokenRefreshShortCircuitReason::MissingRefreshMaterial,
        ),
    ];

    for (grant, expected_reason) in cases {
        let snapshot = sample_snapshot(&grant);
        let adapter = FakeAuthRefreshAdapter::new(RefreshOutcome::TransientFailure {
            safe_error: "not-used".to_string(),
        });
        let sink = FakeCommandSink::new(Ok(Some(sample_apply_result(
            TokenGrantState::NeedsRefresh,
            "fp_old",
        ))));
        let mut service = TokenRefreshService::new(adapter.clone(), sink.clone());

        let report = service
            .refresh_grant_at(snapshot, SystemTime::UNIX_EPOCH)
            .expect("short-circuit should not fail");
        assert_eq!(
            report.status,
            TokenRefreshReportStatus::ShortCircuited(expected_reason)
        );
        assert_eq!(report.decision, None);
        assert_eq!(report.command, None);
        assert_eq!(adapter.calls(), 0);
        assert_eq!(sink.calls(), 0);
    }
}

#[test]
fn service_reports_conflict_noop_when_sink_returns_none() {
    let grant = sample_grant(TokenGrantState::NeedsRefresh, Some("refresh-old"));
    let snapshot = sample_snapshot(&grant);
    let adapter = FakeAuthRefreshAdapter::new(RefreshOutcome::Success {
        rotated_material: EncryptedGrantMaterial {
            encrypted_primary: vec![7, 7, 7],
            encrypted_renewal: vec![8, 8, 8],
        },
        key_id: "key_v2".to_string(),
        new_fingerprint: "fp_new".to_string(),
        refreshed_at: SystemTime::UNIX_EPOCH + Duration::from_secs(1),
        expires_at: None,
    });
    let sink = FakeCommandSink::new(Ok(None));
    let mut service = TokenRefreshService::new(adapter.clone(), sink.clone());

    let report = service
        .refresh_grant_at(snapshot, SystemTime::UNIX_EPOCH + Duration::from_secs(5))
        .expect("service refresh should not fail");

    assert_eq!(adapter.calls(), 1);
    assert_eq!(sink.calls(), 1);
    assert_eq!(report.status, TokenRefreshReportStatus::ConflictNoop);
}

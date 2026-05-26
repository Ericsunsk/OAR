use std::time::{Duration, SystemTime};

use oar_core::domain::identity::{TenantId, TokenGrantId, TokenGrantState};
use oar_core::domain::token_refresh::bridge::{
    plan_token_refresh_command, TokenRefreshBridgeError,
};
use oar_core::domain::token_refresh::service::token_refresh_short_circuit_report;
use oar_core::domain::token_refresh::types::{
    EncryptedGrantMaterial, RefreshOutcome, TokenRefreshCommandKind, TokenRefreshDecision,
    TokenRefreshDecisionKind, TokenRefreshReportStatus, TokenRefreshRepositoryCommand,
    TokenRefreshShortCircuitReason,
};

use crate::common::{sample_grant, sample_snapshot};

#[test]
fn decision_bridge_maps_rotate_command_and_preserves_cas_fields() {
    let refreshed_at = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
    let expires_at = SystemTime::UNIX_EPOCH + Duration::from_secs(20);
    let decision = TokenRefreshDecision::RotateGrantCas {
        grant_id: TokenGrantId("grant_01".to_string()),
        tenant_id: TenantId("tenant_01".to_string()),
        expected_fingerprint: "fp_old".to_string(),
        rotated_material: EncryptedGrantMaterial {
            encrypted_primary: vec![1, 2, 3],
            encrypted_renewal: vec![4, 5],
        },
        key_id: "key_v2".to_string(),
        new_fingerprint: "fp_new".to_string(),
        refreshed_at,
        expires_at: Some(expires_at),
    };

    let command = decision
        .into_repository_command_at(SystemTime::UNIX_EPOCH)
        .expect("bridge command");

    match command {
        TokenRefreshRepositoryCommand::RotateGrantCas {
            grant_id,
            tenant_id,
            expected_fingerprint,
            expires_at_ms,
            refreshed_at_ms,
            encrypted_grant_blob,
            grant_key_id,
            new_fingerprint,
        } => {
            assert_eq!(grant_id, TokenGrantId("grant_01".to_string()));
            assert_eq!(tenant_id, TenantId("tenant_01".to_string()));
            assert_eq!(expected_fingerprint, "fp_old");
            assert_eq!(expires_at_ms, Some(20_000));
            assert_eq!(refreshed_at_ms, 10_000);
            assert_eq!(grant_key_id, "key_v2");
            assert_eq!(new_fingerprint, "fp_new");
            assert_eq!(
                encrypted_grant_blob.0,
                vec![0, 0, 0, 3, 1, 2, 3, 0, 0, 0, 2, 4, 5]
            );
        }
        _ => panic!("expected RotateGrantCas command"),
    }
}

#[test]
fn decision_bridge_failure_commands_use_now_and_keep_expected_fingerprint() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(33);
    let needs_refresh = TokenRefreshDecision::MarkNeedsRefresh {
        grant_id: TokenGrantId("grant_01".to_string()),
        tenant_id: TenantId("tenant_01".to_string()),
        expected_fingerprint: "fp_old".to_string(),
        safe_error: "temporarily unavailable".to_string(),
    };
    let reauth = TokenRefreshDecision::MarkReauthRequired {
        grant_id: TokenGrantId("grant_02".to_string()),
        tenant_id: TenantId("tenant_02".to_string()),
        expected_fingerprint: "fp_current".to_string(),
        safe_error: "invalid_grant".to_string(),
    };

    let needs_refresh_command = needs_refresh
        .into_repository_command_at(now)
        .expect("needs refresh command");
    let reauth_command = reauth
        .into_repository_command_at(now)
        .expect("reauth command");

    match needs_refresh_command {
        TokenRefreshRepositoryCommand::MarkNeedsRefresh {
            expected_fingerprint,
            refreshed_at_ms,
            ..
        } => {
            assert_eq!(expected_fingerprint, "fp_old");
            assert_eq!(refreshed_at_ms, 33_000);
        }
        _ => panic!("expected MarkNeedsRefresh command"),
    }

    match reauth_command {
        TokenRefreshRepositoryCommand::MarkReauthRequired {
            expected_fingerprint,
            reauth_required_at_ms,
            ..
        } => {
            assert_eq!(expected_fingerprint, "fp_current");
            assert_eq!(reauth_required_at_ms, 33_000);
        }
        _ => panic!("expected MarkReauthRequired command"),
    }
}

#[test]
fn plan_token_refresh_command_builds_command_and_report_from_one_decision() {
    let grant = sample_grant(TokenGrantState::NeedsRefresh, Some("refresh-old"));
    let snapshot = sample_snapshot(&grant);
    let planned = plan_token_refresh_command(
        &snapshot,
        RefreshOutcome::TransientFailure {
            safe_error: "temporarily unavailable".to_string(),
        },
        SystemTime::UNIX_EPOCH + Duration::from_secs(21),
    )
    .expect("planned command should be built");

    assert_eq!(
        planned.report.decision_kind,
        TokenRefreshDecisionKind::MarkNeedsRefresh
    );
    assert_eq!(
        planned.report.command_kind,
        TokenRefreshCommandKind::MarkNeedsRefresh
    );
    assert_eq!(
        planned.report.safe_error.as_deref(),
        Some("temporarily unavailable")
    );

    match planned.command {
        TokenRefreshRepositoryCommand::MarkNeedsRefresh {
            grant_id,
            tenant_id,
            expected_fingerprint,
            refreshed_at_ms,
            safe_error,
        } => {
            assert_eq!(grant_id, TokenGrantId("grant_01".to_string()));
            assert_eq!(tenant_id, TenantId("tenant_01".to_string()));
            assert_eq!(expected_fingerprint, "fp_old");
            assert_eq!(refreshed_at_ms, 21_000);
            assert_eq!(safe_error, "temporarily unavailable");
        }
        other => panic!("expected MarkNeedsRefresh command, got {other:?}"),
    }
}

#[test]
fn short_circuit_report_is_the_standard_non_adapter_report() {
    let grant = sample_grant(TokenGrantState::ReauthRequired, Some("refresh-old"));
    let snapshot = sample_snapshot(&grant);

    let report = token_refresh_short_circuit_report(&snapshot).expect("should short-circuit");

    assert_eq!(
        report.status,
        TokenRefreshReportStatus::ShortCircuited(TokenRefreshShortCircuitReason::ReauthRequired)
    );
    assert!(!report.adapter_called);
    assert!(!report.sink_called);
    assert_eq!(report.decision, None);
    assert_eq!(report.command, None);
}

#[test]
fn decision_bridge_rejects_pre_epoch_timestamps() {
    let decision = TokenRefreshDecision::MarkNeedsRefresh {
        grant_id: TokenGrantId("grant_01".to_string()),
        tenant_id: TenantId("tenant_01".to_string()),
        expected_fingerprint: "fp_old".to_string(),
        safe_error: "temporarily unavailable".to_string(),
    };

    let before_epoch = SystemTime::UNIX_EPOCH - Duration::from_secs(1);
    let err = decision
        .into_repository_command_at(before_epoch)
        .expect_err("pre-epoch should fail");
    assert_eq!(err, TokenRefreshBridgeError::TimestampBeforeUnixEpoch);
}

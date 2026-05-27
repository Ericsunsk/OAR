use std::time::SystemTime;

use oar_core::domain::identity::{TenantId, TokenGrantId, TokenGrantState};
use oar_core::domain::token_refresh::decision::{decide_token_refresh, is_refreshable};
use oar_core::domain::token_refresh::types::{
    RefreshOutcome, TokenRefreshAttempt, TokenRefreshDecision, TokenRefreshRepositoryCommand,
};

use crate::common::{
    config_required_outcome, reauth_failure_outcome, sample_attempt, sample_grant,
    sample_rotated_material, success_outcome, transient_failure_outcome,
};

#[test]
fn decide_success_requires_cas_rotation_payload() {
    let now = SystemTime::UNIX_EPOCH;
    let attempt = sample_attempt(success_outcome(now, None));

    let decision = decide_token_refresh(attempt);

    assert_eq!(
        decision,
        TokenRefreshDecision::RotateGrantCas {
            grant_id: TokenGrantId("grant_01".to_string()),
            tenant_id: TenantId("tenant_01".to_string()),
            expected_fingerprint: "fp_old".to_string(),
            rotated_material: sample_rotated_material(),
            key_id: "key_v2".to_string(),
            new_fingerprint: "fp_new".to_string(),
            refreshed_at: now,
            expires_at: None,
        }
    );
}

#[test]
fn decide_transient_failure_marks_needs_refresh() {
    let attempt = sample_attempt(transient_failure_outcome("temporarily unavailable"));

    let decision = decide_token_refresh(attempt);
    assert_eq!(
        decision,
        TokenRefreshDecision::MarkNeedsRefresh {
            grant_id: TokenGrantId("grant_01".to_string()),
            tenant_id: TenantId("tenant_01".to_string()),
            expected_fingerprint: "fp_old".to_string(),
            safe_error: "temporarily unavailable".to_string(),
        }
    );
}

#[test]
fn decide_failure_preserves_allowlisted_safe_error_code() {
    let attempt = sample_attempt(transient_failure_outcome("temporarily unavailable"));

    let decision = decide_token_refresh(attempt);

    assert_eq!(decision.safe_error(), Some("temporarily unavailable"));
}

#[test]
fn decide_reauth_failure_marks_reauth_required() {
    let attempt = sample_attempt(reauth_failure_outcome("invalid_grant"));

    let decision = decide_token_refresh(attempt);
    assert_eq!(
        decision,
        TokenRefreshDecision::MarkReauthRequired {
            grant_id: TokenGrantId("grant_01".to_string()),
            tenant_id: TenantId("tenant_01".to_string()),
            expected_fingerprint: "fp_old".to_string(),
            safe_error: "invalid_grant".to_string(),
        }
    );
}

#[test]
fn decide_config_required_marks_config_required_without_transient_retry() {
    let attempt = sample_attempt(config_required_outcome("refresh_config_required"));

    let decision = decide_token_refresh(attempt);
    assert_eq!(
        decision,
        TokenRefreshDecision::MarkConfigRequired {
            grant_id: TokenGrantId("grant_01".to_string()),
            tenant_id: TenantId("tenant_01".to_string()),
            expected_fingerprint: "fp_old".to_string(),
            safe_error: "refresh_config_required".to_string(),
        }
    );

    let command = decision
        .into_repository_command_at(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(9))
        .expect("config required command should be safe to build");
    match command {
        TokenRefreshRepositoryCommand::MarkConfigRequired {
            refreshed_at_ms,
            safe_error,
            ..
        } => {
            assert_eq!(refreshed_at_ms, 9_000);
            assert_eq!(safe_error, "refresh_config_required");
        }
        other => panic!("expected MarkConfigRequired, got {other:?}"),
    }
}

#[test]
fn revoked_and_reauth_required_grants_are_not_refreshable() {
    let revoked = sample_grant(TokenGrantState::Revoked, Some("refresh-old"));
    let reauth_required = sample_grant(TokenGrantState::ReauthRequired, Some("refresh-old"));

    assert!(!is_refreshable(&revoked));
    assert!(!is_refreshable(&reauth_required));
}

#[test]
fn grant_without_refresh_material_is_not_refreshable() {
    let grant = sample_grant(TokenGrantState::NeedsRefresh, None);
    assert!(!is_refreshable(&grant));
}

#[test]
fn grant_with_revoked_or_reauth_timestamp_is_not_refreshable() {
    let mut revoked_ts = sample_grant(TokenGrantState::Valid, Some("refresh-old"));
    revoked_ts.revoked_at = Some(SystemTime::UNIX_EPOCH);
    assert!(!is_refreshable(&revoked_ts));

    let mut reauth_ts = sample_grant(TokenGrantState::Valid, Some("refresh-old"));
    reauth_ts.reauth_required_at = Some(SystemTime::UNIX_EPOCH);
    assert!(!is_refreshable(&reauth_ts));
}

#[test]
fn decide_failure_redacts_token_like_safe_error_before_report_or_persistence() {
    let attempt = TokenRefreshAttempt {
        grant_id: TokenGrantId("grant_01".to_string()),
        tenant_id: TenantId("tenant_01".to_string()),
        expected_fingerprint: "fp_old".to_string(),
        outcome: RefreshOutcome::TransientFailure {
            safe_error: "eyJhbGciOiJIUzI1NiJ9.fake.payload".to_string(),
        },
    };

    let decision = decide_token_refresh(attempt);

    assert_eq!(decision.safe_error(), Some("<redacted refresh error>"));
    let command = decision
        .into_repository_command_at(SystemTime::UNIX_EPOCH)
        .expect("command should be safe to build");
    match command {
        TokenRefreshRepositoryCommand::MarkNeedsRefresh { safe_error, .. } => {
            assert_eq!(safe_error, "<redacted refresh error>");
        }
        other => panic!("expected MarkNeedsRefresh, got {other:?}"),
    }
}

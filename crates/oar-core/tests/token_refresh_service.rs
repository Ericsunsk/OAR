use std::time::SystemTime;

use oar_core::domain::identity::{
    ActorKind, LarkIdentityId, OAuthTokens, ScopeBoundary, SecretString, TenantId, TokenGrant,
    TokenGrantId, TokenGrantState,
};
use oar_core::domain::token_refresh::{
    decide_token_refresh, is_refreshable, EncryptedGrantMaterial, RefreshOutcome,
    TokenRefreshAttempt, TokenRefreshDecision,
};

fn sample_grant(state: TokenGrantState, refresh_token: Option<&str>) -> TokenGrant {
    TokenGrant {
        id: TokenGrantId("grant_01".to_string()),
        tenant_id: TenantId("tenant_01".to_string()),
        identity_id: LarkIdentityId("identity_01".to_string()),
        actor_kind: ActorKind::User,
        scope_boundary: ScopeBoundary::User,
        scopes: vec!["offline_access".to_string()],
        state,
        issued_at: SystemTime::UNIX_EPOCH,
        expires_at: Some(SystemTime::UNIX_EPOCH),
        refreshed_at: None,
        revoked_at: None,
        reauth_required_at: None,
        last_refresh_error: None,
        tokens: OAuthTokens {
            access_token: SecretString::new("access-old"),
            refresh_token: refresh_token.map(SecretString::new),
        },
        revocation_reason: None,
    }
}

#[test]
fn decide_success_requires_cas_rotation_payload() {
    let now = SystemTime::UNIX_EPOCH;
    let attempt = TokenRefreshAttempt {
        grant_id: TokenGrantId("grant_01".to_string()),
        tenant_id: TenantId("tenant_01".to_string()),
        expected_fingerprint: "fp_old".to_string(),
        outcome: RefreshOutcome::Success {
            rotated_material: EncryptedGrantMaterial {
                encrypted_primary: vec![1, 2, 3],
                encrypted_renewal: vec![4, 5, 6],
            },
            key_id: "key_v2".to_string(),
            new_fingerprint: "fp_new".to_string(),
            refreshed_at: now,
            expires_at: None,
        },
    };

    let decision = decide_token_refresh(attempt);

    assert_eq!(
        decision,
        TokenRefreshDecision::RotateGrantCas {
            grant_id: TokenGrantId("grant_01".to_string()),
            tenant_id: TenantId("tenant_01".to_string()),
            expected_fingerprint: "fp_old".to_string(),
            rotated_material: EncryptedGrantMaterial {
                encrypted_primary: vec![1, 2, 3],
                encrypted_renewal: vec![4, 5, 6],
            },
            key_id: "key_v2".to_string(),
            new_fingerprint: "fp_new".to_string(),
            refreshed_at: now,
            expires_at: None,
        }
    );
}

#[test]
fn decide_transient_failure_marks_needs_refresh() {
    let attempt = TokenRefreshAttempt {
        grant_id: TokenGrantId("grant_01".to_string()),
        tenant_id: TenantId("tenant_01".to_string()),
        expected_fingerprint: "fp_old".to_string(),
        outcome: RefreshOutcome::TransientFailure {
            safe_error: "temporarily unavailable".to_string(),
        },
    };

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
fn decide_reauth_failure_marks_reauth_required() {
    let attempt = TokenRefreshAttempt {
        grant_id: TokenGrantId("grant_01".to_string()),
        tenant_id: TenantId("tenant_01".to_string()),
        expected_fingerprint: "fp_old".to_string(),
        outcome: RefreshOutcome::ReauthFailure {
            safe_error: "invalid_grant".to_string(),
        },
    };

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

use std::time::SystemTime;

use oar_core::domain::identity::{
    ActorKind, LarkIdentityId, OAuthTokens, ScopeBoundary, SecretString, TenantId, TokenGrant,
    TokenGrantId, TokenGrantState,
};
use oar_core::domain::token_refresh::{
    decide_token_refresh, is_refreshable, EncryptedGrantMaterial, RefreshOutcome,
    TokenRefreshAttempt, TokenRefreshBridgeError, TokenRefreshDecision,
    TokenRefreshRepositoryCommand,
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

#[test]
fn decision_bridge_maps_rotate_command_and_preserves_cas_fields() {
    let refreshed_at = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(10);
    let expires_at = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(20);
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
    let now = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(33);
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
fn decision_bridge_rejects_pre_epoch_timestamps() {
    let decision = TokenRefreshDecision::MarkNeedsRefresh {
        grant_id: TokenGrantId("grant_01".to_string()),
        tenant_id: TenantId("tenant_01".to_string()),
        expected_fingerprint: "fp_old".to_string(),
        safe_error: "temporarily unavailable".to_string(),
    };

    let before_epoch = SystemTime::UNIX_EPOCH - std::time::Duration::from_secs(1);
    let err = decision
        .into_repository_command_at(before_epoch)
        .expect_err("pre-epoch should fail");
    assert_eq!(err, TokenRefreshBridgeError::TimestampBeforeUnixEpoch);
}

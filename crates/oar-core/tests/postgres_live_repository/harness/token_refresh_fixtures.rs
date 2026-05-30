use super::{
    ActorKind, EncryptedTokenGrantRecord, RotateEncryptedGrantRequest, ScopeBoundary,
    TokenGrantState, TokenRefreshCommandKind, TokenRefreshCommandReport, TokenRefreshDecisionKind,
    TokenRefreshPlannedCommand, TokenRefreshRepositoryCommand,
};

pub(crate) fn assert_no_auth_refresh_sensitive_payload(payload_text: &str) {
    for needle in [
        "tok_",
        "access_token",
        "refresh_token",
        "authorization_code",
        "Authorization",
        "Bearer",
        "encrypted_primary",
        "encrypted_renewal",
        "fp_prev_v1",
        "fp_rotated_v2",
    ] {
        assert!(
            !payload_text.contains(needle),
            "audit payload leaked auth refresh marker: {needle}"
        );
    }
}

pub(crate) fn encrypted_token_grant_record(
    tenant_id: &str,
    id: &str,
    identity_id: &str,
    state: TokenGrantState,
    fingerprint: &str,
) -> EncryptedTokenGrantRecord {
    EncryptedTokenGrantRecord {
        id: id.to_string(),
        tenant_id: tenant_id.to_string(),
        identity_id: identity_id.to_string(),
        actor_kind: ActorKind::User,
        scope_boundary: ScopeBoundary::User,
        scopes: vec!["okr.progress.write".to_string()],
        state,
        issued_at_ms: 1_748_250_000_000,
        expires_at_ms: Some(1_748_260_000_000),
        refreshed_at_ms: Some(1_748_250_000_000),
        revoked_at_ms: None,
        reauth_required_at_ms: None,
        last_refresh_error: Some("old-error".to_string()),
        encrypted_oauth_grant: vec![0x01, 0x02, 0x03],
        oauth_grant_key_id: "key-v1".to_string(),
        oauth_grant_fingerprint: fingerprint.to_string(),
        revocation_reason: None,
    }
}

pub(crate) fn rotate_grant_request<'a>(
    tenant_id: &'a str,
    id: &'a str,
    expected_fingerprint: &'a str,
    encrypted_oauth_grant: &'a [u8],
) -> RotateEncryptedGrantRequest<'a> {
    RotateEncryptedGrantRequest {
        tenant_id,
        id,
        expected_fingerprint,
        expires_at_ms: Some(1_748_270_000_000),
        refreshed_at_ms: 1_748_260_500_000,
        encrypted_oauth_grant,
        oauth_grant_key_id: "key-v2",
        oauth_grant_fingerprint: "fp-new",
    }
}

pub(crate) fn planned_token_refresh_command(
    command: TokenRefreshRepositoryCommand,
) -> TokenRefreshPlannedCommand {
    let (grant_id, tenant_id) = match &command {
        TokenRefreshRepositoryCommand::RotateGrantCas {
            grant_id,
            tenant_id,
            ..
        }
        | TokenRefreshRepositoryCommand::MarkNeedsRefresh {
            grant_id,
            tenant_id,
            ..
        }
        | TokenRefreshRepositoryCommand::MarkReauthRequired {
            grant_id,
            tenant_id,
            ..
        }
        | TokenRefreshRepositoryCommand::MarkConfigRequired {
            grant_id,
            tenant_id,
            ..
        } => (grant_id.clone(), tenant_id.clone()),
    };
    let command_kind = command.kind();
    let safe_error = match &command {
        TokenRefreshRepositoryCommand::MarkNeedsRefresh { safe_error, .. }
        | TokenRefreshRepositoryCommand::MarkReauthRequired { safe_error, .. }
        | TokenRefreshRepositoryCommand::MarkConfigRequired { safe_error, .. } => {
            Some(safe_error.clone())
        }
        TokenRefreshRepositoryCommand::RotateGrantCas { .. } => None,
    };

    TokenRefreshPlannedCommand {
        command,
        report: TokenRefreshCommandReport {
            grant_id,
            tenant_id,
            decision_kind: match command_kind {
                TokenRefreshCommandKind::RotateGrantCas => TokenRefreshDecisionKind::RotateGrantCas,
                TokenRefreshCommandKind::MarkNeedsRefresh => {
                    TokenRefreshDecisionKind::MarkNeedsRefresh
                }
                TokenRefreshCommandKind::MarkReauthRequired => {
                    TokenRefreshDecisionKind::MarkReauthRequired
                }
                TokenRefreshCommandKind::MarkConfigRequired => {
                    TokenRefreshDecisionKind::MarkConfigRequired
                }
            },
            command_kind,
            safe_error,
        },
    }
}

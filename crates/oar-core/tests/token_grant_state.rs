use std::time::SystemTime;

use oar_core::domain::identity::{
    ActorKind, LarkIdentityId, OAuthTokens, ScopeBoundary, SecretString, TenantId, TokenGrant,
    TokenGrantId, TokenGrantState,
};
use oar_core::domain::token_grant::TokenGrantError;

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
fn refresh_success_rotates_both_tokens_atomically() {
    let grant = sample_grant(TokenGrantState::NeedsRefresh, Some("refresh-old"));
    let now = SystemTime::UNIX_EPOCH;

    let rotated = grant
        .refresh_succeeded_with_rotation(
            now,
            SecretString::new("access-new"),
            SecretString::new("refresh-new"),
            None,
        )
        .expect("rotation should succeed");

    assert_eq!(rotated.state, TokenGrantState::Valid);
    assert_eq!(rotated.tokens.access_token.expose(), "access-new");
    assert_eq!(
        rotated
            .tokens
            .refresh_token
            .as_ref()
            .expect("refresh token should exist")
            .expose(),
        "refresh-new"
    );
}

#[test]
fn refresh_fails_when_refresh_token_missing() {
    let grant = sample_grant(TokenGrantState::NeedsRefresh, None);
    let result = grant.refresh_succeeded_with_rotation(
        SystemTime::UNIX_EPOCH,
        SecretString::new("access-new"),
        SecretString::new("refresh-new"),
        None,
    );

    assert_eq!(result, Err(TokenGrantError::MissingRefreshToken));
}

#[test]
fn revoked_grant_blocks_refresh() {
    let grant = sample_grant(TokenGrantState::Valid, Some("refresh-old"))
        .revoke(SystemTime::UNIX_EPOCH, "user disconnected app")
        .expect("revoke should succeed");

    let result = grant.refresh_succeeded_with_rotation(
        SystemTime::UNIX_EPOCH,
        SecretString::new("access-new"),
        SecretString::new("refresh-new"),
        None,
    );

    assert_eq!(result, Err(TokenGrantError::RevokedGrant));
}

#[test]
fn debug_output_redacts_token_values() {
    let grant = sample_grant(TokenGrantState::Valid, Some("refresh-super-secret"));
    let debug_output = format!("{grant:?}");
    assert!(debug_output.contains("[REDACTED]"));
    assert!(!debug_output.contains("access-old"));
    assert!(!debug_output.contains("refresh-super-secret"));
}

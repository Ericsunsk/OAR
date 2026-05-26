use std::time::SystemTime;

use oar_core::domain::identity::{
    ActorKind, LarkIdentityId, OAuthTokens, ScopeBoundary, SecretString, TenantId, TokenGrant,
    TokenGrantId, TokenGrantState,
};

#[test]
fn token_grant_debug_redacts_token_values() {
    let grant = TokenGrant {
        id: TokenGrantId("grant_01".to_string()),
        tenant_id: TenantId("tenant_01".to_string()),
        identity_id: LarkIdentityId("lark_identity_01".to_string()),
        actor_kind: ActorKind::User,
        scope_boundary: ScopeBoundary::User,
        scopes: vec![
            "offline_access".to_string(),
            "auth:user.id:read".to_string(),
        ],
        state: TokenGrantState::Valid,
        issued_at: SystemTime::UNIX_EPOCH,
        expires_at: None,
        refreshed_at: None,
        revoked_at: None,
        reauth_required_at: None,
        last_refresh_error: None,
        tokens: OAuthTokens {
            access_token: SecretString::new("access-secret-never-log"),
            refresh_token: Some(SecretString::new("refresh-secret-never-log")),
        },
        revocation_reason: None,
    };

    let debug_output = format!("{grant:?}");
    assert!(debug_output.contains("[REDACTED]"));
    assert!(!debug_output.contains("access-secret-never-log"));
    assert!(!debug_output.contains("refresh-secret-never-log"));
}

use std::time::SystemTime;

use oar_core::domain::identity::{
    ActorKind, DeviceSession, DeviceSessionId, DeviceType, LarkIdentityId, OAuthTokens, OarUserId,
    ScopeBoundary, SecretString, SyncCursor, TenantId, TokenGrant, TokenGrantId, TokenGrantState,
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

#[test]
fn device_session_includes_sync_cursor_and_session_identity() {
    let session = DeviceSession {
        id: DeviceSessionId("session_01".to_string()),
        tenant_id: TenantId("tenant_01".to_string()),
        user_id: OarUserId("user_01".to_string()),
        device_type: DeviceType::MacDesktop,
        device_label: "Founder's MacBook".to_string(),
        session_identity: "sid_abc123".to_string(),
        sync_cursor: SyncCursor {
            stream: "okr_review_inbox".to_string(),
            cursor: "cursor-2026w21-42".to_string(),
            updated_at: SystemTime::UNIX_EPOCH,
        },
        last_seen_at: SystemTime::UNIX_EPOCH,
        revoked_at: None,
    };

    assert_eq!(session.session_identity, "sid_abc123");
    assert_eq!(session.sync_cursor.stream, "okr_review_inbox");
    assert_eq!(session.sync_cursor.cursor, "cursor-2026w21-42");
}

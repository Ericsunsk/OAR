use super::support::{contains_bytes, sample_feishu_login};
use crate::feishu_auth::{build_feishu_login_persistence_plan, FeishuLoginPersistenceError};
use oar_core::domain::identity::{ScopeBoundary, TokenGrantState};
use std::time::{Duration, UNIX_EPOCH};

#[test]
fn feishu_login_persistence_plan_builds_stable_redacted_grant() {
    let login = sample_feishu_login(Some("refresh-token-sensitive"));
    let plan = build_feishu_login_persistence_plan(
        &login,
        "oar_session_abc",
        "key-prod-v1",
        [7; 32],
        UNIX_EPOCH + Duration::from_secs(1),
    )
    .expect("plan");

    assert_eq!(plan.tenant.id.0, "feishu_tenant_tenant_1");
    assert_eq!(plan.user.id.0, "feishu_user_tenant_1_ou_123");
    assert_eq!(plan.identity.actor_external_id, "ou_123");
    assert_eq!(plan.grant.identity_id, plan.identity.id.0);
    assert_eq!(plan.grant.scope_boundary, ScopeBoundary::User);
    assert_eq!(
        plan.grant.scopes,
        vec!["auth:user.id:read", "offline_access"]
    );
    assert_eq!(plan.grant.state, TokenGrantState::Valid);
    assert_eq!(plan.grant.issued_at_ms, 1_000);
    assert!(plan.grant.refreshed_at_ms.is_some());
    assert!(plan.grant.expires_at_ms.is_some());
    assert!(plan.grant.encrypted_oauth_grant.len() > "access-token-sensitive".len());
    assert_eq!(plan.session.id.0, "oar_session_abc");
    assert_eq!(plan.session_identity_hash.len(), 64);

    let grant_debug = format!("{:?}", plan.grant);
    assert!(!grant_debug.contains("access-token-sensitive"));
    assert!(!grant_debug.contains("refresh-token-sensitive"));
    assert!(!grant_debug.contains("key-prod-v1"));
    assert!(!grant_debug.contains(&plan.grant.oauth_grant_fingerprint));
    assert!(!contains_bytes(
        &plan.grant.encrypted_oauth_grant,
        b"access-token-sensitive"
    ));
    assert!(!contains_bytes(
        &plan.grant.encrypted_oauth_grant,
        b"refresh-token-sensitive"
    ));
}

#[test]
fn feishu_login_persistence_plan_requires_refresh_token() {
    let login = sample_feishu_login(None);
    let error = build_feishu_login_persistence_plan(
        &login,
        "oar_session_abc",
        "key-prod-v1",
        [7; 32],
        UNIX_EPOCH,
    )
    .expect_err("refresh token required");

    assert_eq!(error, FeishuLoginPersistenceError::MissingRefreshToken);
}

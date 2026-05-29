use std::time::SystemTime;

use oar_core::action::capability::all_capabilities;
use oar_core::action::confirmed_action::{ActionStatus, ConfirmedAction};
use oar_core::action::execution_policy::{ActionActorBinding, ExecutionDenied, ExecutionPolicy};
use oar_core::domain::identity::{
    ActorKind, LarkIdentityId, OAuthTokens, ScopeBoundary, SecretString, TenantId, TokenGrant,
    TokenGrantId, TokenGrantState,
};

fn confirmed_action() -> ConfirmedAction {
    ConfirmedAction::proposed("action-1", "tenant-1", "user-1", "idem-1")
        .confirm(SystemTime::UNIX_EPOCH)
}

fn token_grant(scopes: &[&str], state: TokenGrantState) -> TokenGrant {
    TokenGrant {
        id: TokenGrantId("grant-1".to_string()),
        tenant_id: TenantId("tenant-1".to_string()),
        identity_id: LarkIdentityId("identity-1".to_string()),
        actor_kind: ActorKind::User,
        scope_boundary: ScopeBoundary::User,
        scopes: scopes.iter().map(|scope| (*scope).to_string()).collect(),
        state,
        issued_at: SystemTime::UNIX_EPOCH,
        expires_at: None,
        refreshed_at: None,
        revoked_at: None,
        reauth_required_at: None,
        last_refresh_error: None,
        tokens: OAuthTokens {
            access_token: SecretString::new("access-token"),
            refresh_token: Some(SecretString::new("refresh-token")),
        },
        revocation_reason: None,
    }
}

fn policy() -> ExecutionPolicy {
    ExecutionPolicy::from_capabilities(all_capabilities(), [ActorKind::User, ActorKind::Service])
}

fn actor_binding(actor_user_id: &str, identity_id: &str) -> ActionActorBinding {
    ActionActorBinding::new(
        actor_user_id.to_string(),
        LarkIdentityId(identity_id.to_string()),
    )
}

#[test]
fn allows_confirmed_allowlisted_action_with_required_scope_and_valid_grant() {
    let action = confirmed_action();
    let grant = token_grant(
        &["offline_access", "okr.progress.write"],
        TokenGrantState::Valid,
    );

    let binding = actor_binding("user-1", "identity-1");
    let result = policy().evaluate(
        &action,
        "okr.progress.update",
        "okr.progress.write",
        &grant,
        &binding,
    );

    assert_eq!(result, Ok(()));
}

#[test]
fn allows_progress_create_from_capability_matrix_write_allowlist() {
    let action = confirmed_action();
    let grant = token_grant(&["okr.progress.write"], TokenGrantState::Valid);

    let binding = actor_binding("user-1", "identity-1");
    let result = policy().evaluate(
        &action,
        "okr.progress.create",
        "okr.progress.write",
        &grant,
        &binding,
    );

    assert_eq!(result, Ok(()));
}

#[test]
fn read_capabilities_are_not_added_to_write_execution_allowlist() {
    let action = confirmed_action();
    let grant = token_grant(&["okr.progress.read"], TokenGrantState::Valid);

    let binding = actor_binding("user-1", "identity-1");
    let result = policy().evaluate(
        &action,
        "okr.progress.read",
        "okr.progress.read",
        &grant,
        &binding,
    );

    assert_eq!(
        result,
        Err(ExecutionDenied::ActionNotAllowlisted {
            action_type: "okr.progress.read".to_string()
        })
    );
}

#[test]
fn rejects_when_required_scope_is_missing() {
    let action = confirmed_action();
    let grant = token_grant(&["offline_access"], TokenGrantState::Valid);

    let binding = actor_binding("user-1", "identity-1");
    let result = policy().evaluate(
        &action,
        "okr.progress.update",
        "okr.progress.write",
        &grant,
        &binding,
    );

    assert_eq!(
        result,
        Err(ExecutionDenied::MissingScope {
            required_scope: "okr.progress.write".to_string()
        })
    );
}

#[test]
fn rejects_revoked_token_grant() {
    let action = confirmed_action();
    let grant = token_grant(&["okr.progress.write"], TokenGrantState::Revoked);

    let binding = actor_binding("user-1", "identity-1");
    let result = policy().evaluate(
        &action,
        "okr.progress.update",
        "okr.progress.write",
        &grant,
        &binding,
    );

    assert_eq!(
        result,
        Err(ExecutionDenied::GrantNotExecutable {
            state: TokenGrantState::Revoked
        })
    );
}

#[test]
fn rejects_non_confirmed_action() {
    let action = ConfirmedAction::proposed("action-1", "tenant-1", "user-1", "idem-1");
    let grant = token_grant(&["okr.progress.write"], TokenGrantState::Valid);

    let binding = actor_binding("user-1", "identity-1");
    let result = policy().evaluate(
        &action,
        "okr.progress.update",
        "okr.progress.write",
        &grant,
        &binding,
    );

    assert_eq!(
        result,
        Err(ExecutionDenied::ActionNotConfirmed {
            status: ActionStatus::Proposed
        })
    );
}

#[test]
fn rejects_non_allowlisted_action_type() {
    let action = confirmed_action();
    let grant = token_grant(&["okr.progress.write"], TokenGrantState::Valid);

    let binding = actor_binding("user-1", "identity-1");
    let result = policy().evaluate(
        &action,
        "okr.progress.delete",
        "okr.progress.write",
        &grant,
        &binding,
    );

    assert_eq!(
        result,
        Err(ExecutionDenied::ActionNotAllowlisted {
            action_type: "okr.progress.delete".to_string()
        })
    );
}

#[test]
fn rejects_cross_tenant_grant() {
    let action = confirmed_action();
    let mut grant = token_grant(&["okr.progress.write"], TokenGrantState::Valid);
    grant.tenant_id = TenantId("tenant-other".to_string());

    let binding = actor_binding("user-1", "identity-1");
    let result = policy().evaluate(
        &action,
        "okr.progress.update",
        "okr.progress.write",
        &grant,
        &binding,
    );

    assert_eq!(
        result,
        Err(ExecutionDenied::TenantMismatch {
            action_tenant_id: "tenant-1".to_string(),
            grant_tenant_id: "tenant-other".to_string(),
        })
    );
}

#[test]
fn rejects_when_actor_binding_identity_mismatches_grant_identity() {
    let action = confirmed_action();
    let grant = token_grant(&["okr.progress.write"], TokenGrantState::Valid);
    let binding = actor_binding("user-1", "identity-other");

    let result = policy().evaluate(
        &action,
        "okr.progress.update",
        "okr.progress.write",
        &grant,
        &binding,
    );

    assert_eq!(
        result,
        Err(ExecutionDenied::IdentityMismatch {
            action_actor_user_id: "user-1".to_string(),
            grant_identity_id: "identity-1".to_string(),
            bound_identity_id: "identity-other".to_string(),
        })
    );
}

#[test]
fn rejects_when_actor_binding_user_mismatches_action_actor() {
    let action = confirmed_action();
    let grant = token_grant(&["okr.progress.write"], TokenGrantState::Valid);
    let binding = actor_binding("user-other", "identity-1");

    let result = policy().evaluate(
        &action,
        "okr.progress.update",
        "okr.progress.write",
        &grant,
        &binding,
    );

    assert_eq!(
        result,
        Err(ExecutionDenied::ActorUserMismatch {
            action_actor_user_id: "user-1".to_string(),
            bound_actor_user_id: "user-other".to_string(),
        })
    );
}

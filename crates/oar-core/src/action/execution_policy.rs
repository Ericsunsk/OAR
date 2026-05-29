use std::collections::HashSet;

use crate::action::capability::CapabilitySpec;
use crate::action::confirmed_action::{ActionStatus, ConfirmedAction};
use crate::domain::identity::{ActorKind, LarkIdentityId, TokenGrant, TokenGrantState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionPolicy {
    allowlisted_action_types: HashSet<String>,
    allowed_actor_kinds: Vec<ActorKind>,
}

impl ExecutionPolicy {
    pub fn new(
        allowlisted_action_types: impl IntoIterator<Item = impl Into<String>>,
        allowed_actor_kinds: impl IntoIterator<Item = ActorKind>,
    ) -> Self {
        Self {
            allowlisted_action_types: allowlisted_action_types
                .into_iter()
                .map(Into::into)
                .collect(),
            allowed_actor_kinds: allowed_actor_kinds.into_iter().collect(),
        }
    }

    /// Builds an execution write allowlist from capability specs.
    pub fn from_capabilities<'a>(
        capabilities: impl IntoIterator<Item = &'a CapabilitySpec>,
        allowed_actor_kinds: impl IntoIterator<Item = ActorKind>,
    ) -> Self {
        Self::new(
            capabilities
                .into_iter()
                .filter(|capability| capability.enters_execution_allowlist())
                .map(|capability| capability.action_type_str()),
            allowed_actor_kinds,
        )
    }

    pub fn evaluate(
        &self,
        action: &ConfirmedAction,
        action_type: &str,
        required_scope: &str,
        grant: &TokenGrant,
        actor_binding: &ActionActorBinding,
    ) -> Result<(), ExecutionDenied> {
        if grant.tenant_id.0 != action.tenant_id {
            return Err(ExecutionDenied::TenantMismatch {
                action_tenant_id: action.tenant_id.clone(),
                grant_tenant_id: grant.tenant_id.0.clone(),
            });
        }

        if actor_binding.actor_user_id != action.actor_user_id {
            return Err(ExecutionDenied::ActorUserMismatch {
                action_actor_user_id: action.actor_user_id.clone(),
                bound_actor_user_id: actor_binding.actor_user_id.clone(),
            });
        }

        if grant.identity_id != actor_binding.identity_id {
            return Err(ExecutionDenied::IdentityMismatch {
                action_actor_user_id: action.actor_user_id.clone(),
                grant_identity_id: grant.identity_id.0.clone(),
                bound_identity_id: actor_binding.identity_id.0.clone(),
            });
        }

        if action.status != ActionStatus::Confirmed {
            return Err(ExecutionDenied::ActionNotConfirmed {
                status: action.status,
            });
        }

        if !self.allowlisted_action_types.contains(action_type) {
            return Err(ExecutionDenied::ActionNotAllowlisted {
                action_type: action_type.to_string(),
            });
        }

        if !self.allowed_actor_kinds.contains(&grant.actor_kind) {
            return Err(ExecutionDenied::ActorKindNotAllowed {
                actor_kind: grant.actor_kind,
            });
        }

        if matches!(
            grant.state,
            TokenGrantState::Revoked | TokenGrantState::Expired | TokenGrantState::ReauthRequired
        ) {
            return Err(ExecutionDenied::GrantNotExecutable { state: grant.state });
        }

        if !grant.scopes.iter().any(|scope| scope == required_scope) {
            return Err(ExecutionDenied::MissingScope {
                required_scope: required_scope.to_string(),
            });
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionActorBinding {
    pub actor_user_id: String,
    pub identity_id: LarkIdentityId,
}

impl ActionActorBinding {
    pub fn new(actor_user_id: impl Into<String>, identity_id: LarkIdentityId) -> Self {
        Self {
            actor_user_id: actor_user_id.into(),
            identity_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionDenied {
    TenantMismatch {
        action_tenant_id: String,
        grant_tenant_id: String,
    },
    ActorUserMismatch {
        action_actor_user_id: String,
        bound_actor_user_id: String,
    },
    IdentityMismatch {
        action_actor_user_id: String,
        grant_identity_id: String,
        bound_identity_id: String,
    },
    ActionNotConfirmed {
        status: ActionStatus,
    },
    ActionNotAllowlisted {
        action_type: String,
    },
    ActorKindNotAllowed {
        actor_kind: ActorKind,
    },
    GrantNotExecutable {
        state: TokenGrantState,
    },
    MissingScope {
        required_scope: String,
    },
}

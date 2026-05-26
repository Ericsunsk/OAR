use std::collections::HashSet;

use crate::action::confirmed_action::{ActionStatus, ConfirmedAction};
use crate::domain::identity::{ActorKind, TokenGrant, TokenGrantState};

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

    pub fn evaluate(
        &self,
        action: &ConfirmedAction,
        action_type: &str,
        required_scope: &str,
        grant: &TokenGrant,
    ) -> Result<(), ExecutionDenied> {
        if grant.tenant_id.0 != action.tenant_id {
            return Err(ExecutionDenied::TenantMismatch {
                action_tenant_id: action.tenant_id.clone(),
                grant_tenant_id: grant.tenant_id.0.clone(),
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
pub enum ExecutionDenied {
    TenantMismatch {
        action_tenant_id: String,
        grant_tenant_id: String,
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

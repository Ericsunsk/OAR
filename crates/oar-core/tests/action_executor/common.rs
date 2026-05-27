use std::cell::RefCell;
use std::rc::Rc;
use std::time::SystemTime;

use oar_core::action::audit_event::AuditStateSummary;
use oar_core::action::confirmed_action::ConfirmedAction;
use oar_core::action::execution_policy::{ActionActorBinding, ExecutionPolicy};
use oar_core::action::executor::{ActionAdapter, AdapterDryRun, AdapterError, AdapterExecution};
use oar_core::domain::identity::{
    ActorKind, LarkIdentityId, OAuthTokens, ScopeBoundary, SecretString, TenantId, TokenGrant,
    TokenGrantId, TokenGrantState,
};

pub fn confirmed_action(idempotency_key: &str) -> ConfirmedAction {
    ConfirmedAction::proposed("action-1", "tenant-1", "user-1", idempotency_key)
        .confirm(SystemTime::UNIX_EPOCH)
}

pub fn token_grant(scopes: &[&str], state: TokenGrantState) -> TokenGrant {
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

pub fn actor_binding(actor_user_id: &str) -> ActionActorBinding {
    ActionActorBinding::new(actor_user_id, LarkIdentityId("identity-1".to_string()))
}

pub fn progress_update_policy() -> ExecutionPolicy {
    ExecutionPolicy::new(
        ["okr.progress.update"],
        [ActorKind::User, ActorKind::Service],
    )
}

#[derive(Clone)]
pub struct MockAdapter {
    state: Rc<RefCell<MockState>>,
}

#[derive(Default)]
struct MockState {
    dry_run_calls: usize,
    execute_calls: usize,
    execute_error: Option<AdapterError>,
}

impl MockAdapter {
    pub fn new() -> Self {
        Self {
            state: Rc::new(RefCell::new(MockState::default())),
        }
    }

    pub fn with_execute_error(code: &str, message: &str) -> Self {
        let adapter = Self::new();
        adapter.state.borrow_mut().execute_error =
            Some(AdapterError::from_safe_message(code, message));
        adapter
    }

    pub fn dry_run_calls(&self) -> usize {
        self.state.borrow().dry_run_calls
    }

    pub fn execute_calls(&self) -> usize {
        self.state.borrow().execute_calls
    }
}

impl ActionAdapter for MockAdapter {
    fn dry_run(&mut self, _action: &ConfirmedAction) -> Result<AdapterDryRun, AdapterError> {
        self.state.borrow_mut().dry_run_calls += 1;
        Ok(AdapterDryRun {
            before: Some(AuditStateSummary {
                summary: "before".to_string(),
                reference_ids: vec!["evidence-1".to_string()],
                content_hash: None,
            }),
            after: Some(AuditStateSummary {
                summary: "dry-run after".to_string(),
                reference_ids: vec!["evidence-1".to_string()],
                content_hash: None,
            }),
        })
    }

    fn execute(&mut self, _action: &ConfirmedAction) -> Result<AdapterExecution, AdapterError> {
        let mut state = self.state.borrow_mut();
        state.execute_calls += 1;
        if let Some(error) = state.execute_error.clone() {
            return Err(error);
        }
        Ok(AdapterExecution {
            adapter_operation_id: "lark-op-1".to_string(),
            before: Some(AuditStateSummary {
                summary: "before".to_string(),
                reference_ids: vec!["evidence-1".to_string()],
                content_hash: None,
            }),
            after: Some(AuditStateSummary {
                summary: "applied".to_string(),
                reference_ids: vec!["evidence-1".to_string()],
                content_hash: None,
            }),
        })
    }
}

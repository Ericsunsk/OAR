use std::cell::RefCell;
use std::rc::Rc;
use std::time::SystemTime;

use oar_core::domain::identity::{
    ActorKind, LarkIdentityId, OAuthTokens, ScopeBoundary, SecretString, TenantId, TokenGrant,
    TokenGrantId, TokenGrantState,
};
use oar_core::domain::token_refresh::service::{AuthRefreshAdapter, TokenRefreshCommandSink};
use oar_core::domain::token_refresh::types::{
    RefreshOutcome, TokenRefreshApplyResult, TokenRefreshGrantSnapshot,
    TokenRefreshRepositoryCommand,
};

pub(crate) fn sample_grant(state: TokenGrantState, refresh_token: Option<&str>) -> TokenGrant {
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

pub(crate) fn sample_snapshot(grant: &TokenGrant) -> TokenRefreshGrantSnapshot {
    TokenRefreshGrantSnapshot::from_grant(grant, "fp_old")
}

#[derive(Clone)]
pub(crate) struct FakeAuthRefreshAdapter {
    state: Rc<RefCell<FakeAuthRefreshState>>,
}

#[derive(Clone)]
struct FakeAuthRefreshState {
    calls: usize,
    outcome: RefreshOutcome,
}

impl FakeAuthRefreshAdapter {
    pub(crate) fn new(outcome: RefreshOutcome) -> Self {
        Self {
            state: Rc::new(RefCell::new(FakeAuthRefreshState { calls: 0, outcome })),
        }
    }

    pub(crate) fn calls(&self) -> usize {
        self.state.borrow().calls
    }
}

impl AuthRefreshAdapter for FakeAuthRefreshAdapter {
    fn refresh(&mut self, _snapshot: &TokenRefreshGrantSnapshot) -> RefreshOutcome {
        let mut state = self.state.borrow_mut();
        state.calls += 1;
        state.outcome.clone()
    }
}

#[derive(Clone)]
pub(crate) struct FakeCommandSink {
    state: Rc<RefCell<FakeCommandSinkState>>,
}

#[derive(Clone)]
struct FakeCommandSinkState {
    calls: usize,
    last_command: Option<TokenRefreshRepositoryCommand>,
    result: Result<Option<TokenRefreshApplyResult>, ()>,
}

impl FakeCommandSink {
    pub(crate) fn new(result: Result<Option<TokenRefreshApplyResult>, ()>) -> Self {
        Self {
            state: Rc::new(RefCell::new(FakeCommandSinkState {
                calls: 0,
                last_command: None,
                result,
            })),
        }
    }

    pub(crate) fn calls(&self) -> usize {
        self.state.borrow().calls
    }

    pub(crate) fn last_command(&self) -> Option<TokenRefreshRepositoryCommand> {
        self.state.borrow().last_command.clone()
    }
}

impl TokenRefreshCommandSink for FakeCommandSink {
    type Error = ();

    fn apply_refresh_command(
        &mut self,
        command: TokenRefreshRepositoryCommand,
    ) -> Result<Option<TokenRefreshApplyResult>, Self::Error> {
        let mut state = self.state.borrow_mut();
        state.calls += 1;
        state.last_command = Some(command);
        state.result.clone()
    }
}

pub(crate) fn sample_apply_result(
    state: TokenGrantState,
    fingerprint: &str,
) -> TokenRefreshApplyResult {
    TokenRefreshApplyResult {
        grant_id: TokenGrantId("grant_01".to_string()),
        tenant_id: TenantId("tenant_01".to_string()),
        state,
        fingerprint: fingerprint.to_string(),
    }
}

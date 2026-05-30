use super::*;
use crate::AuthenticatedContext;
use oar_core::action::capability::FeishuScope;
use oar_core::domain::token_refresh::service::{
    AuthRefreshAdapter, TokenRefreshCommandSink, TokenRefreshService,
};
use oar_core::domain::token_refresh::types::{
    EncryptedGrantMaterial, RefreshOutcome, TokenRefreshApplyResult, TokenRefreshRepositoryCommand,
};
use std::cell::RefCell;
use std::rc::Rc;

mod live_read;
mod refresh;

fn refresh_if_stale_for_test<A, S>(
    grant: &EncryptedTokenGrantRecord,
    now: SystemTime,
    adapter: A,
    sink: S,
) -> Option<TokenRefreshServiceReport>
where
    A: AuthRefreshAdapter,
    S: TokenRefreshCommandSink,
    S::Error: std::fmt::Debug,
{
    if !grant_requires_refresh_before_read(grant, system_time_to_ms(now)) {
        return None;
    }
    let snapshot = token_refresh_snapshot_for_live_read(grant);
    let mut service = TokenRefreshService::new(adapter, sink);
    Some(
        service
            .refresh_grant_at(snapshot, now)
            .expect("test token refresh service"),
    )
}

fn sample_token_grant_record(
    state: TokenGrantState,
    expires_at_ms: Option<u64>,
) -> EncryptedTokenGrantRecord {
    EncryptedTokenGrantRecord {
        id: "grant_01".to_string(),
        tenant_id: "tenant_01".to_string(),
        identity_id: "identity_01".to_string(),
        actor_kind: ActorKind::User,
        scope_boundary: ScopeBoundary::User,
        scopes: vec![FeishuScope::OkrProgressRead.as_str().to_string()],
        state,
        issued_at_ms: 1,
        expires_at_ms,
        refreshed_at_ms: None,
        revoked_at_ms: None,
        reauth_required_at_ms: None,
        last_refresh_error: None,
        encrypted_oauth_grant: vec![1, 2, 3],
        oauth_grant_key_id: "grant_key_v1".to_string(),
        oauth_grant_fingerprint: "fp_old".to_string(),
        revocation_reason: None,
    }
}

#[derive(Clone)]
struct FakeAuthRefreshAdapter {
    state: Rc<RefCell<FakeAuthRefreshState>>,
}

struct FakeAuthRefreshState {
    calls: usize,
    outcome: RefreshOutcome,
}

impl FakeAuthRefreshAdapter {
    fn new(outcome: RefreshOutcome) -> Self {
        Self {
            state: Rc::new(RefCell::new(FakeAuthRefreshState { calls: 0, outcome })),
        }
    }

    fn calls(&self) -> usize {
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
struct FakeCommandSink {
    state: Rc<RefCell<FakeCommandSinkState>>,
}

struct FakeCommandSinkState {
    calls: usize,
    result: Result<Option<TokenRefreshApplyResult>, ()>,
    last_command: Option<TokenRefreshRepositoryCommand>,
}

impl FakeCommandSink {
    fn new(result: Result<Option<TokenRefreshApplyResult>, ()>) -> Self {
        Self {
            state: Rc::new(RefCell::new(FakeCommandSinkState {
                calls: 0,
                result,
                last_command: None,
            })),
        }
    }

    fn calls(&self) -> usize {
        self.state.borrow().calls
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

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

#[test]
fn live_read_refresh_trace_id_does_not_embed_session_or_grant() {
    let auth_context = AuthenticatedContext {
        session_id: "oar_session_secret".to_string(),
        tenant_id: "tenant_x".to_string(),
        user_id: "feishu_user_secret".to_string(),
    };

    let trace_id = safe_live_read_trace_id(&auth_context, "grant_secret", 42);

    assert!(trace_id.starts_with("live-feishu-read-"));
    assert!(!trace_id.contains("oar_session_secret"));
    assert!(!trace_id.contains("grant_secret"));
    assert!(!trace_id.contains("feishu_user_secret"));
}

#[test]
fn expired_grant_triggers_refresh_path_before_live_read() {
    let now = UNIX_EPOCH + Duration::from_secs(10);
    let now_ms = system_time_to_ms(now);
    let grant = sample_token_grant_record(TokenGrantState::Valid, Some(now_ms - 1));
    let adapter = FakeAuthRefreshAdapter::new(RefreshOutcome::Success {
        rotated_material: EncryptedGrantMaterial {
            encrypted_primary: vec![1, 2, 3],
            encrypted_renewal: vec![4, 5, 6],
        },
        key_id: "key_v2".to_string(),
        new_fingerprint: "fp_new".to_string(),
        refreshed_at: now,
        expires_at: Some(now + Duration::from_secs(3600)),
    });
    let sink = FakeCommandSink::new(Ok(Some(TokenRefreshApplyResult {
        grant_id: TokenGrantId(grant.id.clone()),
        tenant_id: TenantId(grant.tenant_id.clone()),
        state: TokenGrantState::Valid,
        fingerprint: "fp_new".to_string(),
    })));

    let report =
        refresh_if_stale_for_test(&grant, now, adapter.clone(), sink.clone()).expect("refresh");

    assert_eq!(adapter.calls(), 1);
    assert_eq!(sink.calls(), 1);
    assert_eq!(
        report.command,
        Some(TokenRefreshCommandKind::RotateGrantCas)
    );
    assert!(ensure_refresh_report_allows_read(&report).is_ok());
}

#[test]
fn grant_refresh_predicate_uses_expiry_skew() {
    let now = UNIX_EPOCH + Duration::from_secs(10);
    let now_ms = system_time_to_ms(now);

    let inside_skew =
        sample_token_grant_record(TokenGrantState::Valid, Some(now_ms + TOKEN_REFRESH_SKEW_MS));
    assert!(grant_requires_refresh_before_read(&inside_skew, now_ms));

    let outside_skew = sample_token_grant_record(
        TokenGrantState::Valid,
        Some(now_ms + TOKEN_REFRESH_SKEW_MS + 1),
    );
    assert!(!grant_requires_refresh_before_read(&outside_skew, now_ms));
}

#[test]
fn unusable_grant_states_deny_live_read_even_without_timestamps() {
    let revoked = sample_token_grant_record(TokenGrantState::Revoked, None);
    assert_eq!(
        live_read_grant_denial_reason(&revoked),
        Some("授权已失效，需要重新登录")
    );

    let reauth = sample_token_grant_record(TokenGrantState::ReauthRequired, None);
    assert_eq!(
        live_read_grant_denial_reason(&reauth),
        Some("授权已失效，需要重新登录")
    );

    let mut bot = sample_token_grant_record(TokenGrantState::Valid, None);
    bot.actor_kind = ActorKind::Bot;
    assert_eq!(
        live_read_grant_denial_reason(&bot),
        Some("授权主体不是当前用户")
    );
}

#[test]
fn refresh_failure_safely_degrades_live_read() {
    let now = UNIX_EPOCH + Duration::from_secs(10);
    let now_ms = system_time_to_ms(now);
    let grant = sample_token_grant_record(TokenGrantState::Expired, Some(now_ms - 1));
    let adapter = FakeAuthRefreshAdapter::new(RefreshOutcome::TransientFailure {
        safe_error: "raw-access-token-sensitive".to_string(),
    });
    let sink = FakeCommandSink::new(Ok(Some(TokenRefreshApplyResult {
        grant_id: TokenGrantId(grant.id.clone()),
        tenant_id: TenantId(grant.tenant_id.clone()),
        state: TokenGrantState::NeedsRefresh,
        fingerprint: "fp_old".to_string(),
    })));

    let report =
        refresh_if_stale_for_test(&grant, now, adapter.clone(), sink.clone()).expect("refresh");
    let error = ensure_refresh_report_allows_read(&report).expect_err("degrade");
    let debug = format!("{report:?} {error:?}");

    assert_eq!(adapter.calls(), 1);
    assert_eq!(sink.calls(), 1);
    assert_eq!(
        report.command,
        Some(TokenRefreshCommandKind::MarkNeedsRefresh)
    );
    assert_eq!(error.safe_reason(), "授权令牌刷新失败");
    assert!(!debug.contains("raw-access-token-sensitive"));
}

#[test]
fn grant_debug_redacts_token_material() {
    let mut grant = sample_token_grant_record(TokenGrantState::Valid, None);
    grant.encrypted_oauth_grant =
        b"access-token-sensitive refresh-token-sensitive raw-response".to_vec();

    let debug = format!("{grant:?}");

    assert!(!debug.contains("access-token-sensitive"));
    assert!(!debug.contains("refresh-token-sensitive"));
    assert!(!debug.contains("raw-response"));
}

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

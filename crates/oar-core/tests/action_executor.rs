use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::SystemTime;

use oar_core::action::audit_event::{AuditEventType, AuditStateSummary, ExecutionStatus};
use oar_core::action::confirmed_action::{ActionStatus, ConfirmedAction};
use oar_core::action::execution_policy::{ExecutionDenied, ExecutionPolicy};
use oar_core::action::executor::{
    ActionAdapter, ActionExecutor, AdapterDryRun, AdapterError, AdapterExecution, ExecutionError,
};
use oar_core::domain::identity::{
    ActorKind, LarkIdentityId, OAuthTokens, ScopeBoundary, SecretString, TenantId, TokenGrant,
    TokenGrantId, TokenGrantState,
};
use oar_core::lark::adapter::MockLarkAdapter;

fn confirmed_action(idempotency_key: &str) -> ConfirmedAction {
    ConfirmedAction::proposed("action-1", "tenant-1", "user-1", idempotency_key)
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

fn progress_update_policy() -> ExecutionPolicy {
    ExecutionPolicy::new(
        ["okr.progress.update"],
        [ActorKind::User, ActorKind::Service],
    )
}

#[derive(Clone)]
struct MockAdapter {
    state: Rc<RefCell<MockState>>,
}

#[derive(Default)]
struct MockState {
    dry_run_calls: usize,
    execute_calls: usize,
    execute_error: Option<AdapterError>,
}

impl MockAdapter {
    fn new() -> Self {
        Self {
            state: Rc::new(RefCell::new(MockState::default())),
        }
    }

    fn with_execute_error(code: &str, message: &str) -> Self {
        let adapter = Self::new();
        adapter.state.borrow_mut().execute_error = Some(AdapterError::new(code, message));
        adapter
    }

    fn dry_run_calls(&self) -> usize {
        self.state.borrow().dry_run_calls
    }

    fn execute_calls(&self) -> usize {
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
        if let Some(err) = state.execute_error.clone() {
            return Err(err);
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

#[test]
fn executes_confirmed_action_through_ledger_adapter_and_audit() {
    let adapter = MockAdapter::new();
    let mut ticks = VecDeque::from([10_u64, 20, 30]);
    let mut executor =
        ActionExecutor::with_clock(adapter.clone(), move || ticks.pop_front().unwrap_or(999));
    let action = confirmed_action("idem-exec-1");

    let report = executor.execute_confirmed_action(&action).unwrap();

    assert!(!report.duplicate);
    assert_eq!(report.operation.status, ActionStatus::Succeeded);
    assert_eq!(adapter.dry_run_calls(), 1);
    assert_eq!(adapter.execute_calls(), 1);
    assert_eq!(report.events.len(), 3);
    assert_eq!(
        report.events[0].event_type,
        AuditEventType::ConfirmedActionRecorded
    );
    assert_eq!(report.events[1].event_type, AuditEventType::DryRunExecuted);
    assert_eq!(
        report.events[2].event_type,
        AuditEventType::ExecutionSucceeded
    );
    assert_eq!(
        report.events[2]
            .execution
            .as_ref()
            .map(|v| v.status.clone()),
        Some(ExecutionStatus::Succeeded)
    );

    let persisted = executor.audit().find_by_trace_id("trace-idem-exec-1");
    assert_eq!(persisted, report.events);
}

#[test]
fn duplicate_idempotency_key_does_not_execute_adapter_twice() {
    let adapter = MockAdapter::new();
    let mut ticks = VecDeque::from([1_u64, 2, 3, 4, 5, 6]);
    let mut executor =
        ActionExecutor::with_clock(adapter.clone(), move || ticks.pop_front().unwrap_or(999));
    let action = confirmed_action("idem-dup-1");

    let first = executor.execute_confirmed_action(&action).unwrap();
    let second = executor.execute_confirmed_action(&action).unwrap();

    assert_eq!(adapter.dry_run_calls(), 1);
    assert_eq!(adapter.execute_calls(), 1);
    assert!(!first.duplicate);
    assert!(second.duplicate);
    assert_eq!(first.operation.operation_id, second.operation.operation_id);
    assert!(second.events.is_empty());
}

#[test]
fn execute_failure_marks_ledger_failed_and_emits_failure_event() {
    let adapter = MockAdapter::with_execute_error("adapter_timeout", "network timeout");
    let mut ticks = VecDeque::from([100_u64, 200, 300]);
    let mut executor =
        ActionExecutor::with_clock(adapter.clone(), move || ticks.pop_front().unwrap_or(999));
    let action = confirmed_action("idem-fail-1");

    let report = executor.execute_confirmed_action(&action).unwrap();

    assert_eq!(adapter.dry_run_calls(), 1);
    assert_eq!(adapter.execute_calls(), 1);
    assert_eq!(report.operation.status, ActionStatus::Failed);
    assert_eq!(
        report.operation.last_error.as_deref(),
        Some("network timeout")
    );
    assert_eq!(report.events.len(), 3);
    assert_eq!(report.events[2].event_type, AuditEventType::ExecutionFailed);
    assert_eq!(
        report.events[2]
            .execution
            .as_ref()
            .and_then(|v| v.error_code.as_deref()),
        Some("adapter_timeout")
    );

    let persisted = executor.audit().find_by_trace_id("trace-idem-fail-1");
    assert_eq!(persisted, report.events);
}

#[test]
fn mock_lark_adapter_runs_through_action_executor() {
    let adapter = MockLarkAdapter::succeeding();
    let mut ticks = VecDeque::from([1_000_u64, 2_000, 3_000]);
    let mut executor =
        ActionExecutor::with_clock(adapter, move || ticks.pop_front().unwrap_or(9_999));
    let action = confirmed_action("idem-lark-adapter");

    let report = executor.execute_confirmed_action(&action).unwrap();

    assert_eq!(report.operation.status, ActionStatus::Succeeded);
    assert_eq!(report.events.len(), 3);
    assert_eq!(
        report.events[0].event_type,
        AuditEventType::ConfirmedActionRecorded
    );
    assert_eq!(report.events[1].event_type, AuditEventType::DryRunExecuted);
    assert_eq!(
        report.events[2].event_type,
        AuditEventType::ExecutionSucceeded
    );
    assert_eq!(
        report.events[2]
            .execution
            .as_ref()
            .and_then(|execution| execution.adapter_operation_id.as_deref()),
        Some("mock-lark-idem-lark-adapter")
    );
}

#[test]
fn policy_denied_action_does_not_call_adapter_or_mark_success_and_records_safe_reason() {
    let adapter = MockAdapter::new();
    let mut ticks = VecDeque::from([7_u64, 8, 9]);
    let mut executor =
        ActionExecutor::with_clock(adapter.clone(), move || ticks.pop_front().unwrap_or(999));
    let action = confirmed_action("idem-policy-denied-1");
    let policy = progress_update_policy();
    let grant = token_grant(&["offline_access"], TokenGrantState::Valid);

    let result = executor.execute_confirmed_action_with_policy(
        &action,
        "okr.progress.update",
        "okr.progress.write",
        &grant,
        &policy,
    );

    assert_eq!(adapter.dry_run_calls(), 0);
    assert_eq!(adapter.execute_calls(), 0);
    assert!(
        executor
            .ledger()
            .get_by_idempotency_key(&action.idempotency_key)
            .is_none(),
        "policy denial should happen before ledger submission"
    );

    let denial = match result {
        Err(ExecutionError::PolicyDenied(report)) => report,
        other => panic!("expected policy denial, got {other:?}"),
    };

    assert_eq!(
        denial.denial,
        ExecutionDenied::MissingScope {
            required_scope: "okr.progress.write".to_string()
        }
    );
    assert_eq!(denial.events.len(), 1);
    assert_eq!(denial.events[0].event_type, AuditEventType::ExecutionDenied);
    assert_eq!(
        denial.events[0]
            .execution
            .as_ref()
            .map(|v| v.status.clone()),
        Some(ExecutionStatus::Denied)
    );
    assert_eq!(
        denial.events[0]
            .execution
            .as_ref()
            .and_then(|execution| execution.error_code.as_deref()),
        Some("policy_denied")
    );
    let message = denial.events[0]
        .execution
        .as_ref()
        .and_then(|execution| execution.message.as_deref())
        .unwrap_or_default();
    assert!(message.contains("policy"));
    assert!(message.contains("okr.progress.write"));
    assert!(!message.contains("access-token"));
    assert!(!message.contains("refresh-token"));

    let persisted = executor
        .audit()
        .find_by_trace_id("trace-idem-policy-denied-1");
    assert_eq!(persisted, denial.events);
}

#[test]
fn allowed_policy_preserves_happy_path_execution() {
    let adapter = MockAdapter::new();
    let mut ticks = VecDeque::from([11_u64, 22, 33]);
    let mut executor =
        ActionExecutor::with_clock(adapter.clone(), move || ticks.pop_front().unwrap_or(999));
    let action = confirmed_action("idem-policy-allow-1");
    let policy = progress_update_policy();
    let grant = token_grant(&["okr.progress.write"], TokenGrantState::Valid);

    let report = executor
        .execute_confirmed_action_with_policy(
            &action,
            "okr.progress.update",
            "okr.progress.write",
            &grant,
            &policy,
        )
        .unwrap();

    assert_eq!(adapter.dry_run_calls(), 1);
    assert_eq!(adapter.execute_calls(), 1);
    assert!(!report.duplicate);
    assert_eq!(report.operation.status, ActionStatus::Succeeded);
    assert_eq!(report.events.len(), 3);
    assert_eq!(
        report.events[0].event_type,
        AuditEventType::ConfirmedActionRecorded
    );
    assert_eq!(report.events[1].event_type, AuditEventType::DryRunExecuted);
    assert_eq!(
        report.events[2].event_type,
        AuditEventType::ExecutionSucceeded
    );
}

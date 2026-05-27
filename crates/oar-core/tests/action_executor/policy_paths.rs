use std::collections::VecDeque;

use oar_core::action::audit_event::{AuditEventType, ExecutionStatus};
use oar_core::action::confirmed_action::ActionStatus;
use oar_core::action::execution_policy::ExecutionDenied;
use oar_core::action::executor::{ActionExecutor, ExecutionError};
use oar_core::domain::identity::TokenGrantState;

use crate::common::{
    actor_binding, assert_success_event_sequence, confirmed_action, progress_update_policy,
    token_grant, MockAdapter,
};

#[test]
fn policy_denied_action_does_not_call_adapter_or_mark_success_and_records_safe_reason() {
    let adapter = MockAdapter::new();
    let mut ticks = VecDeque::from([7_u64, 8, 9]);
    let mut executor =
        ActionExecutor::with_clock(adapter.clone(), move || ticks.pop_front().unwrap_or(999));
    let action = confirmed_action("idem-policy-denied-1");
    let policy = progress_update_policy();
    let grant = token_grant(&["offline_access"], TokenGrantState::Valid);
    let binding = actor_binding("user-1");

    let result = executor.execute_confirmed_action_with_policy(
        &action,
        "okr.progress.update",
        "okr.progress.write",
        &binding,
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
            .map(|value| value.status.clone()),
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
        .find_by_trace_id("trace-tenant-1-idem-policy-denied-1");
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
    let binding = actor_binding("user-1");

    let report = executor
        .execute_confirmed_action_with_policy(
            &action,
            "okr.progress.update",
            "okr.progress.write",
            &binding,
            &grant,
            &policy,
        )
        .unwrap();

    assert_eq!(adapter.dry_run_calls(), 1);
    assert_eq!(adapter.execute_calls(), 1);
    assert!(!report.duplicate);
    assert_eq!(report.operation.status, ActionStatus::Succeeded);
    assert_success_event_sequence(&report.events);
}

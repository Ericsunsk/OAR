use std::collections::VecDeque;

use oar_core::action::audit_event::{AuditEventType, ExecutionStatus};
use oar_core::action::audit_repository::InMemoryAuditEventRepository;
use oar_core::action::confirmed_action::ActionStatus;
use oar_core::action::executor::ActionExecutor;
use oar_core::action::operation_ledger::SubmitResult;
use oar_core::action::operation_ledger_repository::InMemoryOperationLedgerRepository;
use oar_core::lark::adapter::MockLarkAdapter;

use crate::common::{assert_success_event_sequence, confirmed_action, MockAdapter};

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
    assert_success_event_sequence(&report.events);
    assert_eq!(
        report.events[2]
            .execution
            .as_ref()
            .map(|value| value.status.clone()),
        Some(ExecutionStatus::Succeeded)
    );

    let persisted = executor
        .audit()
        .find_by_trace_id("trace-tenant-1-idem-exec-1");
    assert_eq!(persisted, report.events);
}

#[test]
fn duplicate_terminal_idempotency_key_returns_existing_terminal_result() {
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
    assert_eq!(second.operation.status, ActionStatus::Succeeded);
    assert_eq!(first.operation.operation_id, second.operation.operation_id);
    assert!(second.events.is_empty());
}

#[test]
fn resumes_from_existing_confirmed_record_without_recreating_operation() {
    let adapter = MockAdapter::new();
    let mut ticks = VecDeque::from([11_u64, 12, 13]);
    let action = confirmed_action("idem-resume-confirmed");
    let ledger = InMemoryOperationLedgerRepository::new();
    let created = ledger.submit_confirmed_action(&action).unwrap();
    let expected_operation_id = match created {
        SubmitResult::Created(record) => record.operation_id,
        SubmitResult::Existing(_) => panic!("first submit should create operation"),
    };
    let mut executor = ActionExecutor::with_repositories(
        adapter.clone(),
        move || ticks.pop_front().unwrap_or(999),
        ledger,
        InMemoryAuditEventRepository::new(),
    );

    let report = executor.execute_confirmed_action(&action).unwrap();

    assert!(!report.duplicate);
    assert_eq!(report.operation.operation_id, expected_operation_id);
    assert_eq!(report.operation.status, ActionStatus::Succeeded);
    assert_eq!(adapter.dry_run_calls(), 1);
    assert_eq!(adapter.execute_calls(), 1);
    assert_success_event_sequence(&report.events);
}

#[test]
fn existing_executing_record_is_reported_as_inflight_duplicate() {
    let adapter = MockAdapter::new();
    let action = confirmed_action("idem-resume-executing");
    let ledger = InMemoryOperationLedgerRepository::new();
    ledger.submit_confirmed_action(&action).unwrap();
    ledger.mark_executing(&action.idempotency_key).unwrap();

    let mut executor = ActionExecutor::with_repositories(
        adapter.clone(),
        || 999,
        ledger,
        InMemoryAuditEventRepository::new(),
    );

    let report = executor.execute_confirmed_action(&action).unwrap();

    assert!(report.duplicate);
    assert_eq!(report.operation.status, ActionStatus::Executing);
    assert_eq!(adapter.dry_run_calls(), 0);
    assert_eq!(adapter.execute_calls(), 0);
    assert!(report.events.is_empty());
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
            .and_then(|value| value.error_code.as_deref()),
        Some("adapter_timeout")
    );

    let persisted = executor
        .audit()
        .find_by_trace_id("trace-tenant-1-idem-fail-1");
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
    assert_success_event_sequence(&report.events);
    assert_eq!(
        report.events[2]
            .execution
            .as_ref()
            .and_then(|execution| execution.adapter_operation_id.as_deref()),
        Some("mock-lark-idem-lark-adapter")
    );
}

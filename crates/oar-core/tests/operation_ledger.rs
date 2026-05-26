use std::time::SystemTime;

use oar_core::action::confirmed_action::{ActionStatus, ConfirmedAction};
use oar_core::action::operation_ledger::{LedgerError, OperationLedger, SubmitResult};

fn confirmed_action(idempotency_key: &str) -> ConfirmedAction {
    ConfirmedAction::proposed("action-1", "tenant-1", "user-1", idempotency_key)
        .confirm(SystemTime::UNIX_EPOCH)
}

#[test]
fn duplicate_confirmation_returns_existing_operation_record() {
    let mut ledger = OperationLedger::new();
    let action = confirmed_action("idem-1");

    let first = ledger.submit_confirmed_action(&action).unwrap();
    let second = ledger.submit_confirmed_action(&action).unwrap();

    let first_record = match first {
        SubmitResult::Created(record) => record,
        SubmitResult::Existing(_) => panic!("first submit should create an operation"),
    };
    let second_record = match second {
        SubmitResult::Existing(record) => record,
        SubmitResult::Created(_) => panic!("duplicate submit should return existing operation"),
    };

    assert_eq!(first_record.operation_id, second_record.operation_id);
    assert_eq!(first_record.idempotency_key, second_record.idempotency_key);
    assert_eq!(second_record.status, ActionStatus::Confirmed);
}

#[test]
fn duplicate_execution_attempts_are_deterministic() {
    let mut ledger = OperationLedger::new();
    let action = confirmed_action("idem-2");
    ledger.submit_confirmed_action(&action).unwrap();

    let first_executing = ledger.mark_executing("idem-2").unwrap();
    let second_executing = ledger.mark_executing("idem-2").unwrap();

    assert_eq!(first_executing.operation_id, second_executing.operation_id);
    assert_eq!(second_executing.status, ActionStatus::Executing);

    let succeeded = ledger.mark_succeeded("idem-2").unwrap();
    assert_eq!(succeeded.status, ActionStatus::Succeeded);

    let invalid_retry = ledger.mark_executing("idem-2");
    assert_eq!(
        invalid_retry,
        Err(LedgerError::InvalidTransition {
            from: ActionStatus::Succeeded,
            to: ActionStatus::Executing,
        })
    );
}

#[test]
fn duplicate_failure_keeps_original_error() {
    let mut ledger = OperationLedger::new();
    let action = confirmed_action("idem-failed");
    ledger.submit_confirmed_action(&action).unwrap();
    ledger.mark_executing("idem-failed").unwrap();

    let first_failure = ledger.mark_failed("idem-failed", "adapter timeout").unwrap();
    let second_failure = ledger
        .mark_failed("idem-failed", "different retry error")
        .unwrap();

    assert_eq!(first_failure.operation_id, second_failure.operation_id);
    assert_eq!(second_failure.status, ActionStatus::Failed);
    assert_eq!(second_failure.last_error.as_deref(), Some("adapter timeout"));
}

#[test]
fn unconfirmed_action_is_rejected() {
    let mut ledger = OperationLedger::new();
    let proposed = ConfirmedAction::proposed("action-2", "tenant-1", "user-1", "idem-3");

    let result = ledger.submit_confirmed_action(&proposed);
    assert_eq!(
        result,
        Err(LedgerError::ActionNotConfirmed {
            status: ActionStatus::Proposed
        })
    );
}

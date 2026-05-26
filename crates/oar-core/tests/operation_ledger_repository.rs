use std::sync::Arc;
use std::thread;
use std::time::SystemTime;

use oar_core::action::confirmed_action::{ActionStatus, ConfirmedAction};
use oar_core::action::operation_ledger::{LedgerError, SubmitResult};
use oar_core::action::operation_ledger_repository::InMemoryOperationLedgerRepository;

fn confirmed_action(idempotency_key: &str) -> ConfirmedAction {
    ConfirmedAction::proposed("action-1", "tenant-1", "user-1", idempotency_key)
        .confirm(SystemTime::UNIX_EPOCH)
}

#[test]
fn repository_returns_existing_for_duplicate_idempotency_key() {
    let repo = InMemoryOperationLedgerRepository::new();
    let action = confirmed_action("idem-repo-dup");

    let first = repo.submit_confirmed_action(&action).unwrap();
    let second = repo.submit_confirmed_action(&action).unwrap();

    let first_record = match first {
        SubmitResult::Created(record) => record,
        SubmitResult::Existing(_) => panic!("first submit should create a record"),
    };
    let second_record = match second {
        SubmitResult::Existing(record) => record,
        SubmitResult::Created(_) => panic!("duplicate submit should return existing record"),
    };

    assert_eq!(first_record.operation_id, second_record.operation_id);
    assert_eq!(second_record.status, ActionStatus::Confirmed);
}

#[test]
fn repository_persists_transitions_and_unknown_key_errors() {
    let repo = InMemoryOperationLedgerRepository::new();
    let action = confirmed_action("idem-repo-transition");

    assert_eq!(
        repo.mark_executing("missing-idem"),
        Err(LedgerError::UnknownIdempotencyKey(
            "missing-idem".to_string()
        ))
    );

    repo.submit_confirmed_action(&action).unwrap();
    repo.mark_executing("idem-repo-transition").unwrap();

    let executing = repo
        .get_by_idempotency_key("idem-repo-transition")
        .expect("record should be readable after executing transition");
    assert_eq!(executing.status, ActionStatus::Executing);

    repo.mark_succeeded("idem-repo-transition").unwrap();
    let succeeded = repo
        .get_by_idempotency_key("idem-repo-transition")
        .expect("record should be readable after success transition");
    assert_eq!(succeeded.status, ActionStatus::Succeeded);
}

#[test]
fn concurrent_repository_submissions_create_one_record() {
    let repo = Arc::new(InMemoryOperationLedgerRepository::new());
    let action = Arc::new(confirmed_action("idem-repo-concurrent"));
    let mut handles = Vec::new();

    for _ in 0..8 {
        let repo = Arc::clone(&repo);
        let action = Arc::clone(&action);
        handles.push(thread::spawn(move || {
            repo.submit_confirmed_action(&action)
                .expect("submit should succeed")
        }));
    }

    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().expect("worker thread should finish"))
        .collect();

    let created = results
        .iter()
        .filter(|result| matches!(result, SubmitResult::Created(_)))
        .count();
    let existing = results
        .iter()
        .filter(|result| matches!(result, SubmitResult::Existing(_)))
        .count();
    let operation_ids: Vec<_> = results
        .iter()
        .map(|result| match result {
            SubmitResult::Created(record) | SubmitResult::Existing(record) => {
                record.operation_id.clone()
            }
        })
        .collect();

    assert_eq!(created, 1);
    assert_eq!(existing, 7);
    assert!(operation_ids.iter().all(|id| id == &operation_ids[0]));
}

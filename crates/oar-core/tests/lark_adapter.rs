use std::time::SystemTime;

use oar_core::action::confirmed_action::ConfirmedAction;
use oar_core::lark::adapter::{
    LarkAdapter, LarkAdapterError, LarkExecutionMode, LarkExecutionRequest, MockLarkAdapter,
    ProgressMutation, ProgressMutationKind,
};

fn request(idempotency_key: &str) -> LarkExecutionRequest {
    let action = ConfirmedAction::proposed(
        "action-mock-1",
        "tenant-mock-1",
        "user-mock-1",
        idempotency_key,
    )
    .confirm(SystemTime::UNIX_EPOCH);

    LarkExecutionRequest {
        confirmed_action: action,
        mutation: ProgressMutation {
            kind: ProgressMutationKind::Update,
            objective_id: "objective_mock_alpha".to_string(),
            key_result_id: "kr_mock_beta".to_string(),
            progress_delta: 5,
            note: Some("weekly check-in".to_string()),
        },
    }
}

#[test]
fn dry_run_returns_structured_non_sensitive_summary() {
    let adapter = MockLarkAdapter::succeeding();
    let summary = adapter.dry_run(&request("idem-dry-run")).unwrap();

    assert_eq!(summary.mode, LarkExecutionMode::DryRun);
    assert_eq!(summary.action_id, "action-mock-1");
    assert_eq!(summary.idempotency_key, "idem-dry-run");
    assert_eq!(summary.progress_delta, 5);
    assert!(summary.accepted);
    assert_eq!(summary.resource_hint, "objectiv:kr_mock_");
    assert_eq!(summary.message, "dry-run accepted");
}

#[test]
fn execute_success_returns_execute_summary() {
    let adapter = MockLarkAdapter::succeeding();
    let summary = adapter.execute(&request("idem-exec-ok")).unwrap();

    assert_eq!(summary.mode, LarkExecutionMode::Execute);
    assert_eq!(summary.idempotency_key, "idem-exec-ok");
    assert!(summary.accepted);
    assert_eq!(summary.message, "executed via mock adapter");
}

#[test]
fn execute_failure_returns_safe_structured_error() {
    let adapter = MockLarkAdapter::failing();
    let err = adapter.execute(&request("idem-exec-fail")).unwrap_err();

    assert_eq!(
        err,
        LarkAdapterError::ExecutionFailed {
            code: "MOCK_EXECUTION_FAILURE".to_string(),
            safe_message: "mock adapter configured to fail".to_string(),
        }
    );
}

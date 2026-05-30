use std::time::SystemTime;

use oar_core::action::confirmed_action::ConfirmedAction;
use oar_core::action::execution_request::{ConfirmedExecutionDecision, ConfirmedExecutionRequest};
use oar_core::domain::proposed_action::ProposedActionKind;
use oar_core::lark::adapter::{
    LarkAdapter, LarkAdapterError, LarkExecutionMode, LarkExecutionRequest, MockLarkAdapter,
    ProgressMutation, ProgressMutationKind,
};
use serde_json::json;

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

fn execution_request(payload: serde_json::Value) -> ConfirmedExecutionRequest {
    ConfirmedExecutionRequest {
        confirmed_action: ConfirmedAction::proposed(
            "action-mock-1",
            "tenant-mock-1",
            "user-mock-1",
            "idem-execution-request",
        )
        .confirm(SystemTime::UNIX_EPOCH),
        proposed_action_id: "proposed-mock-1".to_string(),
        proposed_action_version: 1,
        action_kind: ProposedActionKind::UpdateKrProgress,
        target_user_id: Some("user-mock-1".to_string()),
        owner_user_id: None,
        evidence_ids: vec!["evidence-mock-1".to_string()],
        effective_payload: payload,
        decision: ConfirmedExecutionDecision::Confirm,
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
fn progress_mutation_is_decoded_from_execution_request_payload() {
    let mutation = ProgressMutation::from_execution_request(&execution_request(json!({
        "target": {
            "objective_id": "objective_mock_alpha",
            "kr_id": "kr_mock_beta"
        },
        "mutation": {
            "progress_delta": 5,
            "note": "weekly check-in"
        }
    })))
    .unwrap();

    assert_eq!(mutation.kind, ProgressMutationKind::Update);
    assert_eq!(mutation.objective_id, "objective_mock_alpha");
    assert_eq!(mutation.key_result_id, "kr_mock_beta");
    assert_eq!(mutation.progress_delta, 5);
    assert_eq!(mutation.note.as_deref(), Some("weekly check-in"));
}

#[test]
fn progress_mutation_rejects_payload_without_mutation() {
    let err = ProgressMutation::from_execution_request(&execution_request(json!({
        "target": {
            "objective_id": "objective_mock_alpha",
            "kr_id": "kr_mock_beta"
        }
    })))
    .unwrap_err();

    assert_eq!(
        err,
        LarkAdapterError::UnsupportedAction {
            reason: "missing progress mutation".to_string(),
        }
    );
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

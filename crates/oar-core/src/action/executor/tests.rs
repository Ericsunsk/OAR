use std::time::SystemTime;

use crate::action::audit_event::AuditEventType;
use crate::action::confirmed_action::ConfirmedAction;
use crate::action::execution_request::{ConfirmedExecutionDecision, ConfirmedExecutionRequest};
use crate::domain::proposed_action::ProposedActionKind;
use serde_json::json;

use super::{ActionAdapter, ActionExecutor, AdapterDryRun, AdapterError, AdapterExecution};

#[derive(Debug)]
struct FailingAdapter {
    code: String,
    message: String,
}

fn execution_request(idempotency_key: &str) -> ConfirmedExecutionRequest {
    ConfirmedExecutionRequest {
        confirmed_action: ConfirmedAction::proposed("act-1", "tenant-1", "user-1", idempotency_key)
            .confirm(SystemTime::UNIX_EPOCH),
        proposed_action_id: "act-1".to_string(),
        proposed_action_version: 1,
        action_kind: ProposedActionKind::UpdateKrProgress,
        target_user_id: Some("user-1".to_string()),
        owner_user_id: None,
        evidence_ids: vec!["evidence-1".to_string()],
        effective_payload: json!({
            "target": {
                "objective_id": "objective-1",
                "kr_id": "kr-1"
            },
            "mutation": {
                "progress_delta": 1
            }
        }),
        decision: ConfirmedExecutionDecision::Confirm,
    }
}

impl ActionAdapter for FailingAdapter {
    fn dry_run(
        &mut self,
        _request: &ConfirmedExecutionRequest,
    ) -> Result<AdapterDryRun, AdapterError> {
        Ok(AdapterDryRun {
            before: None,
            after: None,
        })
    }

    fn execute(
        &mut self,
        _request: &ConfirmedExecutionRequest,
    ) -> Result<AdapterExecution, AdapterError> {
        Err(AdapterError::from_safe_message(
            self.code.clone(),
            self.message.clone(),
        ))
    }
}

#[test]
fn adapter_error_sanitizes_token_like_message() {
    let error = AdapterError::from_safe_message(
        "  raw-code ",
        "Authorization: Bearer tok_live_fake refresh_token=rt_live_fake",
    );
    assert_eq!(error.code, "raw-code");
    assert_eq!(error.safe_message, "adapter execution failed");
}

#[test]
fn adapter_error_preserves_non_sensitive_message() {
    let error =
        AdapterError::from_safe_message("adapter.timeout", "adapter timeout while calling lark");
    assert_eq!(error.code, "adapter.timeout");
    assert_eq!(error.safe_message, "adapter timeout while calling lark");
}

#[test]
fn adapter_error_debug_redacts_safe_message() {
    let error =
        AdapterError::from_safe_message("adapter.timeout", "adapter timeout while calling lark");
    let debug = format!("{error:?}");
    assert!(debug.contains("adapter.timeout"));
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("adapter timeout while calling lark"));
}

#[test]
fn execution_failure_records_safe_message_only() {
    let request = execution_request("idem-1");
    let adapter = FailingAdapter {
        code: "execution failed".to_string(),
        message: "stderr: Authorization: Bearer at_secret_value".to_string(),
    };
    let mut executor = ActionExecutor::with_clock(adapter, || 1_u64);

    let report = executor
        .execute_confirmed_request(&request)
        .expect("execution should return report");

    assert_eq!(
        report.operation.last_error,
        Some("adapter execution failed".to_string())
    );
    let failed = report
        .events
        .iter()
        .find(|event| event.event_type == AuditEventType::ExecutionFailed)
        .expect("failed event should exist");
    assert_eq!(
        failed
            .execution
            .as_ref()
            .and_then(|execution| execution.message.clone()),
        Some("adapter execution failed".to_string())
    );
}

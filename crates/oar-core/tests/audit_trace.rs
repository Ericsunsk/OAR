use oar_core::action::audit_event::{
    AuditActor, AuditActorKind, AuditEventType, AuditScope, AuditStateSummary, AuditTarget,
    ExecutionStatus,
};
use oar_core::action::audit_trace::AuditTrace;

fn actor() -> AuditActor {
    AuditActor {
        kind: AuditActorKind::User,
        actor_id: "user_123".to_string(),
        display_name: Some("Reviewer".to_string()),
    }
}

fn scope() -> AuditScope {
    AuditScope {
        tenant_id: "tenant_acme".to_string(),
        workspace_id: Some("workspace_cn".to_string()),
    }
}

fn target() -> AuditTarget {
    AuditTarget {
        resource_type: "okr_progress".to_string(),
        resource_id: "progress_456".to_string(),
        action_type: "update_progress".to_string(),
    }
}

fn summary(text: &str) -> AuditStateSummary {
    AuditStateSummary {
        summary: text.to_string(),
        reference_ids: vec!["evidence_1".to_string()],
        content_hash: Some("sha256:abc123".to_string()),
    }
}

#[test]
fn audit_trace_allocates_sequence_and_reuses_same_trace_id() {
    let mut trace = AuditTrace::new("trace_42");

    let confirmed = trace.confirmed_action(
        1_748_250_000_000,
        actor(),
        scope(),
        target(),
        summary("confirmed"),
    );
    let dry_run = trace.dry_run(
        1_748_250_010_000,
        actor(),
        scope(),
        target(),
        Some(summary("before")),
        Some(summary("projected")),
    );
    let succeeded = trace.execution_succeeded(
        1_748_250_020_000,
        actor(),
        scope(),
        target(),
        Some(summary("before")),
        Some(summary("after")),
        "op_789",
    );

    assert_eq!(trace.trace_id(), "trace_42");

    assert_eq!(confirmed.trace_id, "trace_42");
    assert_eq!(dry_run.trace_id, "trace_42");
    assert_eq!(succeeded.trace_id, "trace_42");

    assert_eq!(confirmed.sequence, 1);
    assert_eq!(dry_run.sequence, 2);
    assert_eq!(succeeded.sequence, 3);
    assert!(confirmed.sequence < dry_run.sequence);
    assert!(dry_run.sequence < succeeded.sequence);

    assert_eq!(
        confirmed.event_type,
        AuditEventType::ConfirmedActionRecorded
    );
    assert_eq!(dry_run.event_type, AuditEventType::DryRunExecuted);
    assert_eq!(succeeded.event_type, AuditEventType::ExecutionSucceeded);
}

#[test]
fn audit_trace_failure_keeps_order_and_status() {
    let mut trace = AuditTrace::new("trace_99");

    let _confirmed = trace.confirmed_action(
        1_748_250_100_000,
        actor(),
        scope(),
        target(),
        summary("confirmed"),
    );
    let failed = trace.execution_failed(
        1_748_250_110_000,
        actor(),
        scope(),
        target(),
        Some(summary("before")),
        None,
        "adapter_timeout",
        "Lark adapter timed out",
    );

    assert_eq!(failed.trace_id, "trace_99");
    assert_eq!(failed.sequence, 2);
    assert_eq!(failed.event_type, AuditEventType::ExecutionFailed);
    assert_eq!(
        failed.execution.as_ref().map(|result| &result.status),
        Some(&ExecutionStatus::Failed)
    );
    assert_eq!(
        failed
            .execution
            .as_ref()
            .and_then(|result| result.error_code.as_deref()),
        Some("adapter_timeout")
    );
}

#[test]
fn audit_trace_policy_denial_is_distinct_from_execution_failure() {
    let mut trace = AuditTrace::new("trace_denied");

    let denied = trace.execution_denied(
        1_748_250_120_000,
        actor(),
        scope(),
        target(),
        "policy_denied",
        "Execution denied by policy: missing required scope okr.progress.write",
    );

    assert_eq!(denied.trace_id, "trace_denied");
    assert_eq!(denied.sequence, 1);
    assert_eq!(denied.event_type, AuditEventType::ExecutionDenied);
    assert_eq!(
        denied.execution.as_ref().map(|result| &result.status),
        Some(&ExecutionStatus::Denied)
    );
}

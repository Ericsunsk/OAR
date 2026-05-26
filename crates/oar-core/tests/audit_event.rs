use oar_core::action::audit_event::{
    AuditActor, AuditActorKind, AuditEvent, AuditEventContext, AuditEventType, AuditScope,
    AuditStateSummary, AuditSubject, AuditTarget, ExecutionStatus,
};

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

fn event_context(
    event_id: &str,
    trace_id: &str,
    sequence: u64,
    occurred_at_ms: u64,
) -> AuditEventContext {
    AuditEventContext {
        event_id: event_id.to_string(),
        trace_id: trace_id.to_string(),
        sequence,
        occurred_at_ms,
        subject: AuditSubject {
            actor: actor(),
            scope: scope(),
            target: target(),
        },
    }
}

#[test]
fn confirmed_action_event_is_traceable() {
    let event = AuditEvent::confirmed_action(
        event_context("evt_1", "trace_1", 1, 1_748_250_000_000),
        summary("User confirmed +5 progress update with evidence refs"),
    );

    assert_eq!(event.event_type, AuditEventType::ConfirmedActionRecorded);
    assert_eq!(event.trace_id, "trace_1");
    assert_eq!(event.sequence, 1);
    assert_eq!(event.actor.actor_id, "user_123");
    assert_eq!(event.scope.tenant_id, "tenant_acme");
    assert_eq!(event.target.resource_id, "progress_456");
    assert!(event.after.is_some());
    assert!(event.execution.is_none());
}

#[test]
fn dry_run_and_execution_events_keep_same_trace_and_order() {
    let dry_run = AuditEvent::dry_run(
        event_context("evt_2", "trace_2", 2, 1_748_250_010_000),
        Some(summary("before state")),
        Some(summary("dry-run projected state")),
    );
    let succeeded = AuditEvent::execution_succeeded(
        event_context("evt_3", "trace_2", 3, 1_748_250_020_000),
        Some(summary("before state")),
        Some(summary("applied state")),
        "op_789",
    );

    assert_eq!(dry_run.trace_id, succeeded.trace_id);
    assert!(dry_run.sequence < succeeded.sequence);
    assert_eq!(
        dry_run.execution.as_ref().map(|v| &v.status),
        Some(&ExecutionStatus::DryRun)
    );
    assert_eq!(
        succeeded.execution.as_ref().map(|v| &v.status),
        Some(&ExecutionStatus::Succeeded)
    );
    assert_eq!(
        succeeded
            .execution
            .as_ref()
            .and_then(|v| v.adapter_operation_id.as_deref()),
        Some("op_789")
    );
}

#[test]
fn policy_denied_event_is_traceable_without_adapter_operation() {
    let denied = AuditEvent::execution_denied(
        event_context("evt_denied", "trace_policy", 2, 1_748_250_025_000),
        "policy_denied",
        "Execution denied by policy: missing required scope okr.progress.write",
    );

    assert_eq!(denied.event_type, AuditEventType::ExecutionDenied);
    assert_eq!(denied.trace_id, "trace_policy");
    assert_eq!(
        denied.execution.as_ref().map(|result| &result.status),
        Some(&ExecutionStatus::Denied)
    );
    assert_eq!(
        denied
            .execution
            .as_ref()
            .and_then(|result| result.adapter_operation_id.as_deref()),
        None
    );
    assert_eq!(
        denied
            .execution
            .as_ref()
            .and_then(|result| result.error_code.as_deref()),
        Some("policy_denied")
    );
}

#[test]
fn serialized_event_contains_no_secret_or_token_fields() {
    let failed = AuditEvent::execution_failed(
        event_context("evt_4", "trace_3", 4, 1_748_250_030_000),
        Some(summary("before state")),
        None,
        "adapter_timeout",
        "Lark adapter timed out",
    );

    let json = serde_json::to_string(&failed).expect("serialize");

    assert!(!json.contains("access_token"));
    assert!(!json.contains("refresh_token"));
    assert!(!json.contains("authorization_code"));
    assert!(!json.contains("secret"));
    assert!(!json.contains("access-secret-never-log"));
    assert!(!json.contains("refresh-secret-never-log"));

    let value: serde_json::Value = serde_json::from_str(&json).expect("json object");
    let object = value.as_object().expect("json object");

    assert!(!object.contains_key("access_token"));
    assert!(!object.contains_key("refresh_token"));
    assert!(!object.contains_key("authorization_code"));
    assert!(!object.contains_key("secret"));
    assert!(object.contains_key("trace_id"));
    assert!(object.contains_key("actor"));
    assert!(object.contains_key("scope"));
    assert!(object.contains_key("target"));
}

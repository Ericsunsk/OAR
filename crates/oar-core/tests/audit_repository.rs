use oar_core::action::audit_event::{
    AuditActor, AuditActorKind, AuditEvent, AuditEventContext, AuditEventType, AuditScope,
    AuditStateSummary, AuditSubject, AuditTarget,
};
use oar_core::action::audit_repository::InMemoryAuditEventRepository;

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

fn target(id: &str) -> AuditTarget {
    AuditTarget {
        resource_type: "okr_progress".to_string(),
        resource_id: id.to_string(),
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
    ts_ms: u64,
    target_id: &str,
) -> AuditEventContext {
    AuditEventContext {
        event_id: event_id.to_string(),
        trace_id: trace_id.to_string(),
        sequence,
        occurred_at_ms: ts_ms,
        subject: AuditSubject {
            actor: actor(),
            scope: scope(),
            target: target(target_id),
        },
    }
}

fn confirmed_event(
    event_id: &str,
    trace_id: &str,
    sequence: u64,
    ts_ms: u64,
    target_id: &str,
) -> AuditEvent {
    AuditEvent::confirmed_action(
        event_context(event_id, trace_id, sequence, ts_ms, target_id),
        summary("confirmed"),
    )
}

#[test]
fn appends_events_and_reads_trace_in_original_sequence_order() {
    let repo = InMemoryAuditEventRepository::new();

    let first = confirmed_event("evt_1", "trace_a", 1, 1_748_250_000_000, "progress_1");
    let second = AuditEvent::dry_run(
        event_context("evt_2", "trace_a", 2, 1_748_250_010_000, "progress_1"),
        Some(summary("before")),
        Some(summary("after_dry_run")),
    );
    let other_trace = confirmed_event("evt_3", "trace_b", 1, 1_748_250_020_000, "progress_2");

    repo.append(first).expect("append first");
    repo.append(second).expect("append second");
    repo.append(other_trace).expect("append other trace");

    let trace_a = repo.find_by_trace_id("trace_a");
    assert_eq!(trace_a.len(), 2);
    assert_eq!(trace_a[0].trace_id, "trace_a");
    assert_eq!(trace_a[1].trace_id, "trace_a");
    assert_eq!(trace_a[0].sequence, 1);
    assert_eq!(trace_a[1].sequence, 2);
    assert_eq!(
        trace_a[0].event_type,
        AuditEventType::ConfirmedActionRecorded
    );
    assert_eq!(trace_a[1].event_type, AuditEventType::DryRunExecuted);
}

#[test]
fn serialized_repository_events_do_not_expose_token_like_fields() {
    let repo = InMemoryAuditEventRepository::new();

    repo.append(AuditEvent::execution_failed(
        event_context(
            "evt_4",
            "trace_sensitive",
            1,
            1_748_250_030_000,
            "progress_3",
        ),
        Some(summary("before")),
        None,
        "adapter_timeout",
        "Lark adapter timed out",
    ))
    .expect("append");

    let events = repo.find_by_trace_id("trace_sensitive");
    let json = serde_json::to_string(&events).expect("serialize events");

    assert!(!json.contains("access_token"));
    assert!(!json.contains("refresh_token"));
    assert!(!json.contains("authorization_code"));
    assert!(!json.contains("id_token"));
    assert!(!json.contains("secret"));
}

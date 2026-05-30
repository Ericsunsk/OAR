use serde_json::json;

use super::{
    AuditActor, AuditActorKind, AuditEventContext, AuditOutboxEnvelope, AuditScope,
    AuditStateSummary, AuditSubject, AuditTarget,
};

pub(crate) fn actor(actor_id: &str) -> AuditActor {
    AuditActor {
        kind: AuditActorKind::User,
        actor_id: actor_id.to_string(),
        display_name: Some("Reviewer".to_string()),
    }
}

pub(crate) fn scope(tenant_id: &str) -> AuditScope {
    AuditScope {
        tenant_id: tenant_id.to_string(),
        workspace_id: None,
    }
}

pub(crate) fn target(resource_id: &str) -> AuditTarget {
    AuditTarget {
        resource_type: "okr_progress".to_string(),
        resource_id: resource_id.to_string(),
        action_type: "update_progress".to_string(),
    }
}

pub(crate) fn summary(text: &str) -> AuditStateSummary {
    AuditStateSummary {
        summary: text.to_string(),
        reference_ids: vec!["evidence_1".to_string()],
        content_hash: Some("sha256:abc123".to_string()),
    }
}

pub(crate) fn audit_context(
    event_id: &str,
    trace_id: &str,
    sequence: u64,
    occurred_at_ms: u64,
    actor_id: &str,
    tenant_id: &str,
    resource_id: &str,
) -> AuditEventContext {
    AuditEventContext {
        event_id: event_id.to_string(),
        trace_id: trace_id.to_string(),
        sequence,
        occurred_at_ms,
        subject: AuditSubject {
            actor: actor(actor_id),
            scope: scope(tenant_id),
            target: target(resource_id),
        },
    }
}

pub(crate) fn outbox_envelope(
    tenant_id: &str,
    trace_id: &str,
    next_attempt_at_ms: u64,
) -> AuditOutboxEnvelope {
    AuditOutboxEnvelope {
        tenant_id: tenant_id.to_string(),
        stream: "audit-events".to_string(),
        aggregate_id: trace_id.to_string(),
        payload: json!({ "trace_id": trace_id }),
        next_attempt_at_ms,
    }
}

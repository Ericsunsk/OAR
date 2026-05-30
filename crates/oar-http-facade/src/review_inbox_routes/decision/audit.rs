use oar_core::action::audit_event::{
    AuditActor, AuditActorKind, AuditEvent, AuditEventContext, AuditScope, AuditStateSummary,
    AuditSubject, AuditTarget,
};
use oar_core::storage::postgres::AuditOutboxEnvelope;
use serde_json::json;

use crate::AuthenticatedContext;

use super::super::dto::ReviewDecisionRequestDto;
use super::super::labels::review_decision_kind;

const AUDIT_OUTBOX_STREAM: &str = "audit-events";

pub(super) fn decision_audit_event(
    context: &AuthenticatedContext,
    request: &ReviewDecisionRequestDto,
    decision_id: &str,
    occurred_at_ms: u64,
) -> AuditEvent {
    let trace_id = format!("review-decision:{decision_id}");
    AuditEvent::proposed_action_decision(
        AuditEventContext {
            event_id: format!("audit:{decision_id}"),
            trace_id,
            sequence: 1,
            occurred_at_ms,
            subject: AuditSubject {
                actor: AuditActor {
                    kind: AuditActorKind::User,
                    actor_id: context.user_id.clone(),
                    display_name: None,
                },
                scope: AuditScope {
                    tenant_id: context.tenant_id.clone(),
                    workspace_id: None,
                },
                target: AuditTarget {
                    resource_type: "proposed_action".to_string(),
                    resource_id: request.action_id.clone(),
                    action_type: review_decision_kind(request.decision).to_string(),
                },
            },
        },
        AuditStateSummary {
            summary: format!(
                "review decision {} recorded",
                review_decision_kind(request.decision)
            ),
            reference_ids: vec![request.action_id.clone(), decision_id.to_string()],
            content_hash: None,
        },
    )
}

pub(super) fn decision_audit_outbox(
    context: &AuthenticatedContext,
    event: &AuditEvent,
    next_attempt_at_ms: u64,
) -> AuditOutboxEnvelope {
    AuditOutboxEnvelope {
        tenant_id: context.tenant_id.clone(),
        stream: AUDIT_OUTBOX_STREAM.to_string(),
        aggregate_id: event.trace_id.clone(),
        payload: json!({
            "event_id": event.event_id,
            "trace_id": event.trace_id,
            "event_type": "ProposedActionDecisionRecorded",
            "tenant_id": context.tenant_id,
            "sequence": event.sequence
        }),
        next_attempt_at_ms,
    }
}

use crate::action::audit_event::{
    AuditActor, AuditActorKind, AuditScope, AuditSubject, AuditTarget,
};
use crate::action::audit_trace::AuditTrace;
use crate::action::confirmed_action::ConfirmedAction;

pub(crate) fn action_audit_trace(action: &ConfirmedAction) -> AuditTrace {
    AuditTrace::new(action_trace_id(action), action_audit_subject(action))
}

pub(crate) fn action_trace_id(action: &ConfirmedAction) -> String {
    format!("trace-{}-{}", action.tenant_id, action.idempotency_key)
}

pub(crate) fn action_audit_subject(action: &ConfirmedAction) -> AuditSubject {
    AuditSubject {
        actor: AuditActor {
            kind: AuditActorKind::User,
            actor_id: action.actor_user_id.clone(),
            display_name: None,
        },
        scope: AuditScope {
            tenant_id: action.tenant_id.clone(),
            workspace_id: None,
        },
        target: AuditTarget {
            resource_type: "confirmed_action".to_string(),
            resource_id: action.action_id.clone(),
            action_type: "execute".to_string(),
        },
    }
}

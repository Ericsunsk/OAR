use crate::action::audit_event::{
    AuditActor, AuditEvent, AuditEventContext, AuditScope, AuditStateSummary, AuditSubject,
    AuditTarget,
};
use crate::domain::token_refresh::types::{
    TokenRefreshAuditSummary, TokenRefreshCommandKind, TokenRefreshReportStatus,
    TokenRefreshShortCircuitReason,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenRefreshAuditContext {
    pub trace_id: String,
    pub sequence: u64,
    pub occurred_at_ms: u64,
    pub actor: AuditActor,
    pub workspace_id: Option<String>,
}

pub fn token_refresh_audit_event(
    context: TokenRefreshAuditContext,
    summary: &TokenRefreshAuditSummary,
) -> AuditEvent {
    let target = token_refresh_target(summary);
    let event_context = AuditEventContext {
        event_id: format!("{}-evt-{}", context.trace_id, context.sequence),
        trace_id: context.trace_id,
        sequence: context.sequence,
        occurred_at_ms: context.occurred_at_ms,
        subject: AuditSubject {
            actor: context.actor,
            scope: AuditScope {
                tenant_id: summary.tenant_id.0.clone(),
                workspace_id: context.workspace_id,
            },
            target,
        },
    };
    let adapter_operation_id = format!("token-refresh:{}", summary.grant_id.0);

    match &summary.status {
        TokenRefreshReportStatus::Succeeded => AuditEvent::execution_succeeded(
            event_context,
            Some(before_summary(summary)),
            Some(after_summary(summary)),
            adapter_operation_id,
        ),
        TokenRefreshReportStatus::ConflictNoop => AuditEvent::execution_failed(
            event_context,
            Some(before_summary(summary)),
            Some(after_summary(summary)),
            "token_refresh_conflict_noop",
            "state conflict noop",
        ),
        TokenRefreshReportStatus::ShortCircuited(reason) => {
            let error_code = match reason {
                TokenRefreshShortCircuitReason::Revoked => "token_refresh_revoked",
                TokenRefreshShortCircuitReason::ReauthRequired => "token_refresh_reauth_required",
                TokenRefreshShortCircuitReason::MissingRefreshMaterial => {
                    "token_refresh_missing_refresh_material"
                }
            };

            AuditEvent::execution_denied(event_context, error_code, short_circuit_message(reason))
        }
    }
}

fn token_refresh_target(summary: &TokenRefreshAuditSummary) -> AuditTarget {
    AuditTarget {
        resource_type: "token_grant".to_string(),
        resource_id: summary.grant_id.0.clone(),
        action_type: action_type(summary),
    }
}

fn action_type(summary: &TokenRefreshAuditSummary) -> String {
    match summary.status {
        TokenRefreshReportStatus::Succeeded => match summary.command {
            Some(TokenRefreshCommandKind::RotateGrantCas) => "token_refresh.rotate",
            Some(TokenRefreshCommandKind::MarkNeedsRefresh) => "token_refresh.mark_needs_refresh",
            Some(TokenRefreshCommandKind::MarkReauthRequired) => {
                "token_refresh.mark_reauth_required"
            }
            None => "token_refresh.refresh",
        },
        TokenRefreshReportStatus::ConflictNoop => "token_refresh.conflict_noop",
        TokenRefreshReportStatus::ShortCircuited(_) => "token_refresh.short_circuit",
    }
    .to_string()
}

fn before_summary(summary: &TokenRefreshAuditSummary) -> AuditStateSummary {
    AuditStateSummary {
        summary: "token_refresh_before".to_string(),
        reference_ids: vec![summary.grant_id.0.clone()],
        content_hash: None,
    }
}

fn after_summary(summary: &TokenRefreshAuditSummary) -> AuditStateSummary {
    let outcome = match summary.status {
        TokenRefreshReportStatus::Succeeded => "succeeded",
        TokenRefreshReportStatus::ConflictNoop => "conflict_noop",
        TokenRefreshReportStatus::ShortCircuited(_) => "short_circuited",
    };
    let detail = summary
        .safe_error
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|safe| format!(" ({safe})"))
        .unwrap_or_default();

    AuditStateSummary {
        summary: format!("token_refresh_after:{outcome}{detail}"),
        reference_ids: vec![summary.grant_id.0.clone()],
        content_hash: None,
    }
}

fn short_circuit_message(reason: &TokenRefreshShortCircuitReason) -> &'static str {
    match reason {
        TokenRefreshShortCircuitReason::Revoked => "grant denied: revoked",
        TokenRefreshShortCircuitReason::ReauthRequired => "grant denied: reauthentication required",
        TokenRefreshShortCircuitReason::MissingRefreshMaterial => {
            "grant denied: missing renewal material"
        }
    }
}

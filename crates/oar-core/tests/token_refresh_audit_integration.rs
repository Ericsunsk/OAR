use oar_core::action::audit_event::{AuditActor, AuditActorKind, AuditEventType, ExecutionStatus};
use oar_core::action::token_refresh_audit::{token_refresh_audit_event, TokenRefreshAuditContext};
use oar_core::domain::identity::{TenantId, TokenGrantId};
use oar_core::domain::token_refresh::types::{
    TokenRefreshAuditSummary, TokenRefreshCommandKind, TokenRefreshDecisionKind,
    TokenRefreshReportStatus, TokenRefreshShortCircuitReason,
};

fn base_context() -> TokenRefreshAuditContext {
    TokenRefreshAuditContext {
        trace_id: "trace_refresh_001".to_string(),
        sequence: 7,
        occurred_at_ms: 1_748_250_100_000,
        actor: AuditActor {
            kind: AuditActorKind::Service,
            actor_id: "svc_token_refresher".to_string(),
            display_name: Some("Token Refresh Worker".to_string()),
        },
        workspace_id: None,
    }
}

fn base_summary() -> TokenRefreshAuditSummary {
    TokenRefreshAuditSummary {
        grant_id: TokenGrantId("grant_abc123".to_string()),
        tenant_id: TenantId("tenant_acme".to_string()),
        status: TokenRefreshReportStatus::Succeeded,
        decision: Some(TokenRefreshDecisionKind::RotateGrantCas),
        command: Some(TokenRefreshCommandKind::RotateGrantCas),
        safe_error: None,
    }
}

#[test]
fn success_rotation_emits_execution_succeeded_event() {
    let event = token_refresh_audit_event(base_context(), &base_summary());

    assert_eq!(event.event_type, AuditEventType::ExecutionSucceeded);
    assert_eq!(event.trace_id, "trace_refresh_001");
    assert_eq!(event.sequence, 7);
    assert_eq!(event.event_id, "trace_refresh_001-evt-7");
    assert_eq!(event.actor.kind, AuditActorKind::Service);
    assert_eq!(event.actor.actor_id, "svc_token_refresher");
    assert_eq!(event.scope.tenant_id, "tenant_acme");
    assert_eq!(event.target.resource_type, "token_grant");
    assert_eq!(event.target.action_type, "token_refresh.rotate");
    assert_eq!(event.target.resource_id, "grant_abc123");
    assert_eq!(
        event.execution.as_ref().map(|v| &v.status),
        Some(&ExecutionStatus::Succeeded)
    );
}

#[test]
fn conflict_noop_emits_execution_failed_event_with_redacted_message() {
    let mut summary = base_summary();
    summary.status = TokenRefreshReportStatus::ConflictNoop;

    let event = token_refresh_audit_event(base_context(), &summary);

    assert_eq!(event.event_type, AuditEventType::ExecutionFailed);
    assert_eq!(
        event
            .execution
            .as_ref()
            .and_then(|v| v.error_code.as_deref()),
        Some("token_refresh_conflict_noop")
    );

    let message = event
        .execution
        .as_ref()
        .and_then(|v| v.message.as_deref())
        .unwrap_or_default()
        .to_lowercase();
    assert!(!message.contains("token"));
    assert!(!message.contains("fingerprint"));
    assert!(!message.contains("blob"));
}

#[test]
fn short_circuit_emits_execution_denied_with_stable_error_code() {
    let mut missing = base_summary();
    missing.status = TokenRefreshReportStatus::ShortCircuited(
        TokenRefreshShortCircuitReason::MissingRefreshMaterial,
    );

    let missing_event = token_refresh_audit_event(base_context(), &missing);
    assert_eq!(missing_event.event_type, AuditEventType::ExecutionDenied);
    assert_eq!(
        missing_event
            .execution
            .as_ref()
            .and_then(|v| v.error_code.as_deref()),
        Some("token_refresh_missing_refresh_material")
    );

    let mut revoked = base_summary();
    revoked.status =
        TokenRefreshReportStatus::ShortCircuited(TokenRefreshShortCircuitReason::Revoked);
    let revoked_event = token_refresh_audit_event(base_context(), &revoked);
    assert_eq!(revoked_event.event_type, AuditEventType::ExecutionDenied);
    assert_eq!(
        revoked_event
            .execution
            .as_ref()
            .and_then(|v| v.error_code.as_deref()),
        Some("token_refresh_revoked")
    );
}

#[test]
fn transient_retry_safe_error_redaction_never_leaks_sensitive_fields() {
    let mut summary = base_summary();
    summary.status = TokenRefreshReportStatus::Succeeded;
    summary.decision = Some(TokenRefreshDecisionKind::MarkNeedsRefresh);
    summary.command = Some(TokenRefreshCommandKind::MarkNeedsRefresh);
    summary.safe_error = Some("<redacted refresh error>".to_string());

    let event = token_refresh_audit_event(base_context(), &summary);
    let json = serde_json::to_string(&event).expect("serialize token refresh audit event");
    let lowered = json.to_lowercase();

    assert!(
        lowered.contains("<redacted refresh error>") || lowered.contains("temporarily unavailable")
    );
    assert!(!lowered.contains("access_token"));
    assert!(!lowered.contains("refresh_token"));
    assert!(!lowered.contains("authorization"));
    assert!(!lowered.contains("encrypted_primary"));
    assert!(!lowered.contains("encrypted_renewal"));
    assert!(!lowered.contains("9, 9, 9"));
}

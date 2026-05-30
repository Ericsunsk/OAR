use std::time::{SystemTime, UNIX_EPOCH};

use hyper::http::StatusCode;
use oar_core::action::audit_event::{
    AuditActor, AuditActorKind, AuditEvent, AuditEventContext, AuditScope, AuditStateSummary,
    AuditSubject, AuditTarget,
};
use oar_core::action::confirmed_action::ConfirmedAction;
use oar_core::domain::identity::{TenantId, WorkspaceUserId};
use oar_core::domain::proposed_action::{
    ProposedAction, ProposedActionDecision, ProposedActionId, ProposedActionKind,
    ProposedActionStatus,
};
use oar_core::domain::review_inbox::{ReviewInboxItem, ReviewInboxItemId, ReviewInboxItemStatus};
use oar_core::storage::postgres::{
    postgres_repository_safe_error_reason, AuditOutboxEnvelope,
    InsertProposedActionDecisionRequest, PostgresReviewDecisionContextRequest,
    PostgresReviewDecisionRecorder, StoredReviewInboxAction, StoredReviewInboxItem,
};
use serde_json::json;

use crate::response::{json_facade_response, service_unavailable, FacadeResponse};
use crate::runtime::OarHttpFacadeRuntime;
use crate::AuthenticatedContext;

use super::dto::{ReviewDecisionKindDto, ReviewDecisionRequestDto};
use super::labels::{action_status, review_decision_kind};
use super::snapshot_for_context;

const AUDIT_OUTBOX_STREAM: &str = "audit-events";
const REVIEW_DECISION_CHANGED_MESSAGE: &str =
    "The review inbox item changed; refresh before retrying.";

pub(super) async fn record_decision_for_context(
    runtime: &OarHttpFacadeRuntime,
    context: &AuthenticatedContext,
    request: ReviewDecisionRequestDto,
) -> FacadeResponse {
    let Some(persistence) = runtime.persistence() else {
        return service_unavailable(
            "review_decision_store_unavailable",
            "Review decision storage is temporarily unavailable.",
        );
    };

    let Some(expected_sync_cursor_value) = request.expected_sync_cursor else {
        return json_facade_response(
            StatusCode::BAD_REQUEST,
            json!({
                "error": "review_decision_missing_sync_cursor",
                "safe_message": "Review decision requires the expected sync cursor."
            }),
        );
    };

    let recorder = PostgresReviewDecisionRecorder::new(persistence.pool());
    let decision_context = match recorder
        .load_review_decision_context(PostgresReviewDecisionContextRequest {
            tenant_id: &context.tenant_id,
            user_id: &context.user_id,
            proposed_action_id: &request.action_id,
            proposed_action_version: request.action_version,
            expected_sync_cursor_value,
        })
        .await
    {
        Ok(Some(context)) => context,
        Ok(None) => {
            return review_decision_conflict(REVIEW_DECISION_CHANGED_MESSAGE);
        }
        Err(_) => {
            return service_unavailable(
                "review_decision_state_unavailable",
                "Review decision state is temporarily unavailable.",
            );
        }
    };

    let item = &decision_context.item;
    if item.status.is_terminal() || item.status != ReviewInboxItemStatus::Open {
        return json_facade_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({
                "error": "review_decision_item_not_open",
                "safe_message": "The requested review item is no longer open."
            }),
        );
    }

    let action = &decision_context.action;
    if action.decision.is_some() {
        return review_decision_conflict("The requested review action already has a decision.");
    }
    if action.status != ProposedActionStatus::Published {
        return json_facade_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({
                "error": "review_decision_action_unsupported",
                "safe_message": "The requested review action cannot be decided."
            }),
        );
    }
    if matches!(
        request.decision,
        ReviewDecisionKindDto::Confirm | ReviewDecisionKindDto::EditThenConfirm
    ) && !is_confirmable_action_kind(&action.kind)
    {
        return review_decision_action_unsupported();
    }

    let now = SystemTime::now();
    let decision = match request.decision {
        ReviewDecisionKindDto::Confirm => ProposedActionDecision::Confirm,
        ReviewDecisionKindDto::EditThenConfirm => ProposedActionDecision::EditThenConfirm {
            edited_payload: request
                .edited_payload
                .clone()
                .unwrap_or_else(|| action.suggested_payload.clone()),
        },
        ReviewDecisionKindDto::Reject => ProposedActionDecision::Reject,
    };
    let mut proposed_action = match proposed_action_from_stored(action) {
        Ok(action) => action,
        Err(_) => {
            return review_decision_action_unsupported();
        }
    };
    let confirmed_action = match proposed_action.decide(decision.clone(), now) {
        Ok(action) => action,
        Err(_) => {
            return review_decision_action_unsupported();
        }
    };
    let next_cursor = item.sync_cursor_value.saturating_add(1);
    let operation_id = confirmed_action.as_ref().map(operation_id);
    let mut inbox_item = review_inbox_item_from_stored(item, now);
    let transition = match confirmed_action.as_ref() {
        Some(_) => inbox_item.confirm(next_cursor, now).map(|()| {
            inbox_item.ledger_status = Some("confirmed".to_string());
            inbox_item.operation_id = operation_id.clone();
        }),
        None => inbox_item.reject(next_cursor, now),
    };
    if transition.is_err() {
        return review_decision_conflict(REVIEW_DECISION_CHANGED_MESSAGE);
    }

    let decision_id = decision_id(&request.action_id, request.action_version, request.decision);
    let confirmed_action_id = confirmed_action
        .as_ref()
        .map(|action| action.action_id.as_str());
    let decided_at_ms = match system_time_to_ms(now) {
        Some(value) => value,
        None => {
            return service_unavailable(
                "review_decision_clock_unavailable",
                "Review decision storage is temporarily unavailable.",
            )
        }
    };
    let audit_event = decision_audit_event(context, &request, &decision_id, decided_at_ms);
    let outbox = decision_audit_outbox(context, &audit_event, decided_at_ms);
    match recorder
        .record_decision(
            oar_core::storage::postgres::PostgresReviewDecisionRecorderRequest {
                expected_sync_cursor_value,
                decision: InsertProposedActionDecisionRequest {
                    id: &decision_id,
                    tenant_id: &context.tenant_id,
                    proposed_action_id: &request.action_id,
                    proposed_action_version: request.action_version,
                    actor_user_id: &context.user_id,
                    decision: &decision,
                    confirmed_action_id,
                    decided_at: now,
                },
                confirmed_action: confirmed_action.as_ref(),
                confirmed_at_ms: confirmed_action.as_ref().map(|_| decided_at_ms),
                operation_id: operation_id.as_deref(),
                inbox_item: &inbox_item,
                event: &audit_event,
                outbox: &outbox,
            },
        )
        .await
    {
        Ok(_) => snapshot_for_context(runtime, context).await,
        Err(error) => {
            let reason = postgres_repository_safe_error_reason(&error);
            if reason == "review_decision_request_mismatch" {
                review_decision_conflict(REVIEW_DECISION_CHANGED_MESSAGE)
            } else {
                service_unavailable(
                    "review_decision_record_failed",
                    "Review decision could not be recorded.",
                )
            }
        }
    }
}

pub(super) fn decode_review_decision_request(
    body: &[u8],
) -> Result<ReviewDecisionRequestDto, FacadeResponse> {
    let request: ReviewDecisionRequestDto = serde_json::from_slice(body).map_err(|_| {
        json_facade_response(
            StatusCode::BAD_REQUEST,
            json!({
                "error": "review_decision_invalid_json",
                "safe_message": "Review decision request body must be valid JSON."
            }),
        )
    })?;
    if request.action_id.trim().is_empty()
        || request.action_version == 0
        || request.note.chars().count() > 320
    {
        return Err(json_facade_response(
            StatusCode::BAD_REQUEST,
            json!({
                "error": "review_decision_invalid_request",
                "safe_message": "Review decision request is invalid."
            }),
        ));
    }
    Ok(request)
}

fn proposed_action_from_stored(
    action: &StoredReviewInboxAction,
) -> Result<ProposedAction, oar_core::domain::proposed_action::ProposedActionError> {
    let mut proposed = ProposedAction::draft(
        ProposedActionId(action.id.clone()),
        TenantId(action.tenant_id.clone()),
        WorkspaceUserId(action.actor_user_id.clone()),
        action.target_user_id.clone().map(WorkspaceUserId),
        action.owner_user_id.clone().map(WorkspaceUserId),
        action.version,
        action.kind.clone(),
        action.risk_severity,
        action.evidence_ids.clone(),
        action.suggested_payload.clone(),
    )?;
    proposed.publish()?;
    Ok(proposed)
}

fn review_inbox_item_from_stored(
    item: &StoredReviewInboxItem,
    updated_at: SystemTime,
) -> ReviewInboxItem {
    ReviewInboxItem {
        id: ReviewInboxItemId(item.id.clone()),
        tenant_id: TenantId(item.tenant_id.clone()),
        user_id: WorkspaceUserId(item.user_id.clone()),
        proposed_action_id: item.proposed_action_id.clone(),
        proposed_action_version: item.proposed_action_version,
        risk_score: item.risk_score,
        priority: item.priority,
        status: item.status,
        sort_key: item.sort_key,
        sync_cursor: item.sync_cursor_value,
        updated_at,
        ledger_status: item.ledger_status.map(action_status).map(str::to_string),
        operation_id: item.operation_id.clone(),
    }
}

fn decision_audit_event(
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

fn decision_audit_outbox(
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

fn review_decision_conflict(safe_message: &'static str) -> FacadeResponse {
    json_facade_response(
        StatusCode::CONFLICT,
        json!({
            "error": "review_decision_conflict",
            "safe_message": safe_message
        }),
    )
}

fn review_decision_action_unsupported() -> FacadeResponse {
    json_facade_response(
        StatusCode::UNPROCESSABLE_ENTITY,
        json!({
            "error": "review_decision_action_unsupported",
            "safe_message": "The requested review action cannot be decided."
        }),
    )
}

fn decision_id(action_id: &str, version: u64, decision: ReviewDecisionKindDto) -> String {
    format!(
        "decision:{}:v{}:{}",
        action_id,
        version,
        review_decision_kind(decision)
    )
}

fn operation_id(action: &ConfirmedAction) -> String {
    format!("op-{}", action.idempotency_key)
}

fn is_confirmable_action_kind(kind: &ProposedActionKind) -> bool {
    matches!(kind, ProposedActionKind::UpdateKrProgress)
}

fn system_time_to_ms(value: SystemTime) -> Option<u64> {
    value
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis() as u64)
}

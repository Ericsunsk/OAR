use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use http_body_util::{BodyExt, LengthLimitError, Limited};
use hyper::body::Incoming;
use hyper::http::{Method, StatusCode};
use oar_core::action::audit_event::{
    AuditActor, AuditActorKind, AuditEvent, AuditEventContext, AuditScope, AuditStateSummary,
    AuditSubject, AuditTarget,
};
use oar_core::action::confirmed_action::{ActionStatus, ConfirmedAction};
use oar_core::domain::evidence::{EvidenceSourceKind, EvidenceVisibilityScope};
use oar_core::domain::identity::{TenantId, WorkspaceUserId};
use oar_core::domain::proposed_action::{ProposedAction, ProposedActionDecision, ProposedActionId};
use oar_core::domain::proposed_action::{ProposedActionKind, ProposedActionStatus, RiskSeverity};
use oar_core::domain::review_inbox::{ReviewInboxItem, ReviewInboxItemId, ReviewInboxItemStatus};
use oar_core::storage::postgres::{
    postgres_repository_safe_error_reason, AuditOutboxEnvelope,
    InsertProposedActionDecisionRequest, PostgresReviewDecisionContextRequest,
    PostgresReviewDecisionRecorder, PostgresReviewInboxRepository,
    StoredProposedActionDecisionKind, StoredReviewInboxAction, StoredReviewInboxEvidence,
    StoredReviewInboxItem, StoredReviewInboxLedgerEvent, StoredReviewInboxLedgerStage,
    StoredReviewInboxLedgerStatus, StoredReviewInboxSnapshot,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::feishu_auth::iso8601_utc;
use crate::response::{json_facade_response, not_found, service_unavailable, FacadeResponse};
use crate::runtime::OarHttpFacadeRuntime;
use crate::{authenticate_oar_session, oar_session_auth_error_response, AuthenticatedContext};

const CONTRACT_VERSION: u64 = 1;
const DEFAULT_SNAPSHOT_LIMIT: u32 = 100;
const REVIEW_DECISIONS_PATH: &str = "/review-inbox/decisions";
const AUDIT_OUTBOX_STREAM: &str = "audit-events";
const REVIEW_DECISION_BODY_LIMIT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize, PartialEq)]
struct ReviewInboxSnapshotDto {
    contract_version: u64,
    generated_at: String,
    items: Vec<ReviewInboxItemDto>,
    proposed_actions: Vec<ProposedActionDto>,
    evidence: Vec<EvidenceItemDto>,
    ledger_events: Vec<LedgerEventDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct ReviewInboxItemDto {
    id: String,
    tenant_id: String,
    user_id: String,
    proposed_action_id: String,
    proposed_action_version: u64,
    objective_title: String,
    key_result_title: String,
    owner_display_name: String,
    week_label: String,
    risk_score: u32,
    priority: u32,
    risk_reason: String,
    confidence_score: f64,
    status: &'static str,
    sync_cursor: u64,
    updated_at_display: String,
    ledger_status: Option<&'static str>,
    operation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct ProposedActionDto {
    id: String,
    review_item_id: String,
    tenant_id: String,
    actor_user_id: String,
    target_user_id: Option<String>,
    owner_user_id: Option<String>,
    version: u64,
    status: &'static str,
    kind: String,
    risk_severity: &'static str,
    evidence_ids: Vec<String>,
    rationale: String,
    expected_impact: String,
    dry_run_result_summary: String,
    estimated_write_targets_count: u64,
    decision: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct EvidenceItemDto {
    id: String,
    review_item_id: String,
    source_kind: &'static str,
    source_id: String,
    locator: Option<String>,
    observed_at_display: String,
    summary: String,
    signal_type: &'static str,
    trust_score: f64,
    content_hash: String,
    visibility: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct LedgerEventDto {
    id: String,
    action_id: String,
    stage: &'static str,
    stage_status: &'static str,
    timestamp_display: String,
    message: String,
    idempotency_key: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
struct ReviewDecisionRequestDto {
    #[serde(rename = "action_id")]
    action_id: String,
    #[serde(rename = "action_version")]
    action_version: u64,
    decision: ReviewDecisionKindDto,
    note: String,
    #[serde(rename = "expected_sync_cursor")]
    expected_sync_cursor: Option<u64>,
    #[serde(default, rename = "edited_payload")]
    edited_payload: Option<Value>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ReviewDecisionKindDto {
    Confirm,
    EditThenConfirm,
    Reject,
}

pub(crate) fn is_body_route(method: &Method, path: &str) -> bool {
    *method == Method::POST && path == REVIEW_DECISIONS_PATH
}

pub(crate) async fn body_route_response(
    runtime: Arc<OarHttpFacadeRuntime>,
    method: &Method,
    path: &str,
    authorization: Option<&str>,
    body: Incoming,
) -> FacadeResponse {
    if !is_body_route(method, path) {
        return not_found();
    }

    let auth_context = match authenticate_oar_session(&runtime, authorization).await {
        Ok(context) => context,
        Err(error) => return oar_session_auth_error_response(error),
    };
    let body = match Limited::new(body, REVIEW_DECISION_BODY_LIMIT_BYTES)
        .collect()
        .await
    {
        Ok(collected) => collected.to_bytes(),
        Err(error) if error.downcast_ref::<LengthLimitError>().is_some() => {
            return json_facade_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                json!({
                    "error": "review_decision_body_too_large",
                    "safe_message": "Review decision request body is too large."
                }),
            );
        }
        Err(_) => {
            return json_facade_response(
                StatusCode::BAD_REQUEST,
                json!({
                    "error": "review_decision_body_unreadable",
                    "safe_message": "Review decision request body could not be read."
                }),
            );
        }
    };
    let request = match decode_review_decision_request(&body) {
        Ok(request) => request,
        Err(response) => return response,
    };

    record_decision_for_context(&runtime, &auth_context, request).await
}

pub(crate) async fn snapshot_for_context(
    runtime: &OarHttpFacadeRuntime,
    context: &AuthenticatedContext,
) -> FacadeResponse {
    let Some(persistence) = runtime
        .feishu_login
        .as_ref()
        .and_then(|login| login.grant_persistence())
    else {
        return service_unavailable(
            "review_inbox_snapshot_store_unavailable",
            "Review inbox snapshot storage is temporarily unavailable.",
        );
    };

    let repository = PostgresReviewInboxRepository::new(persistence.pool());
    match repository
        .load_review_inbox_snapshot(
            &context.tenant_id,
            &context.user_id,
            0,
            DEFAULT_SNAPSHOT_LIMIT,
        )
        .await
    {
        Ok(snapshot) => json_facade_response(
            StatusCode::OK,
            snapshot_response_body(&snapshot, SystemTime::now()),
        ),
        Err(_) => service_unavailable(
            "review_inbox_snapshot_unavailable",
            "Review inbox snapshot is temporarily unavailable.",
        ),
    }
}

async fn record_decision_for_context(
    runtime: &OarHttpFacadeRuntime,
    context: &AuthenticatedContext,
    request: ReviewDecisionRequestDto,
) -> FacadeResponse {
    let Some(persistence) = runtime
        .feishu_login
        .as_ref()
        .and_then(|login| login.grant_persistence())
    else {
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
            return review_decision_conflict(
                "The review inbox item changed; refresh before retrying.",
            );
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
        return json_facade_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({
                "error": "review_decision_action_unsupported",
                "safe_message": "The requested review action cannot be decided."
            }),
        );
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
            return json_facade_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                json!({
                    "error": "review_decision_action_unsupported",
                    "safe_message": "The requested review action cannot be decided."
                }),
            );
        }
    };
    let confirmed_action = match proposed_action.decide(decision.clone(), now) {
        Ok(action) => action,
        Err(_) => {
            return json_facade_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                json!({
                    "error": "review_decision_action_unsupported",
                    "safe_message": "The requested review action cannot be decided."
                }),
            );
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
        return review_decision_conflict("The review inbox item changed; refresh before retrying.");
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
                review_decision_conflict("The review inbox item changed; refresh before retrying.")
            } else {
                service_unavailable(
                    "review_decision_record_failed",
                    "Review decision could not be recorded.",
                )
            }
        }
    }
}

fn snapshot_response_body(snapshot: &StoredReviewInboxSnapshot, generated_at: SystemTime) -> Value {
    serde_json::to_value(snapshot_dto(snapshot, generated_at))
        .expect("review inbox snapshot dto is serializable")
}

fn decode_review_decision_request(body: &[u8]) -> Result<ReviewDecisionRequestDto, FacadeResponse> {
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
                    action_type: decision_kind(request.decision).to_string(),
                },
            },
        },
        AuditStateSummary {
            summary: format!(
                "review decision {} recorded",
                decision_kind(request.decision)
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

fn decision_id(action_id: &str, version: u64, decision: ReviewDecisionKindDto) -> String {
    format!(
        "decision:{}:v{}:{}",
        action_id,
        version,
        decision_kind(decision)
    )
}

fn decision_kind(decision: ReviewDecisionKindDto) -> &'static str {
    match decision {
        ReviewDecisionKindDto::Confirm => "confirm",
        ReviewDecisionKindDto::EditThenConfirm => "edit_then_confirm",
        ReviewDecisionKindDto::Reject => "reject",
    }
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

fn snapshot_dto(
    snapshot: &StoredReviewInboxSnapshot,
    generated_at: SystemTime,
) -> ReviewInboxSnapshotDto {
    let actions_by_item = actions_by_review_item(&snapshot.actions);
    let evidence_by_item = evidence_by_review_item(&snapshot.evidence);

    ReviewInboxSnapshotDto {
        contract_version: CONTRACT_VERSION,
        generated_at: iso8601_utc(generated_at),
        items: snapshot
            .items
            .iter()
            .map(|item| {
                item_dto(
                    item,
                    actions_by_item.get(item.id.as_str()).copied(),
                    evidence_by_item.get(item.id.as_str()).map(Vec::as_slice),
                )
            })
            .collect(),
        proposed_actions: snapshot.actions.iter().map(proposed_action_dto).collect(),
        evidence: snapshot
            .evidence
            .iter()
            .map(|evidence| {
                evidence_dto(
                    evidence,
                    actions_by_item
                        .get(evidence.review_item_id.as_str())
                        .copied()
                        .map(|action| &action.suggested_payload),
                )
            })
            .collect(),
        ledger_events: snapshot
            .ledger_events
            .iter()
            .map(ledger_event_dto)
            .collect(),
    }
}

fn actions_by_review_item(
    actions: &[StoredReviewInboxAction],
) -> HashMap<&str, &StoredReviewInboxAction> {
    actions
        .iter()
        .map(|action| (action.review_item_id.as_str(), action))
        .collect()
}

fn evidence_by_review_item(
    evidence: &[StoredReviewInboxEvidence],
) -> HashMap<&str, Vec<&StoredReviewInboxEvidence>> {
    let mut by_item: HashMap<&str, Vec<&StoredReviewInboxEvidence>> = HashMap::new();
    for evidence in evidence {
        by_item
            .entry(evidence.review_item_id.as_str())
            .or_default()
            .push(evidence);
    }
    by_item
}

fn item_dto(
    item: &StoredReviewInboxItem,
    action: Option<&StoredReviewInboxAction>,
    evidence: Option<&[&StoredReviewInboxEvidence]>,
) -> ReviewInboxItemDto {
    let payload = action.map(|action| &action.suggested_payload);
    ReviewInboxItemDto {
        id: item.id.clone(),
        tenant_id: item.tenant_id.clone(),
        user_id: item.user_id.clone(),
        proposed_action_id: item.proposed_action_id.clone(),
        proposed_action_version: item.proposed_action_version,
        objective_title: whitelisted_payload_string(payload, "objective_title")
            .unwrap_or_else(|| format!("Review item {}", item.id)),
        key_result_title: whitelisted_payload_string(payload, "key_result_title")
            .or_else(|| first_evidence_summary(evidence))
            .unwrap_or_else(|| "Review required".to_string()),
        owner_display_name: whitelisted_payload_string(payload, "owner_display_name")
            .or_else(|| action.and_then(|action| action.owner_user_id.clone()))
            .unwrap_or_else(|| "Unassigned".to_string()),
        week_label: whitelisted_payload_string(payload, "week_label")
            .unwrap_or_else(|| "Current week".to_string()),
        risk_score: item.risk_score,
        priority: item.priority,
        risk_reason: whitelisted_payload_string(payload, "risk_reason")
            .or_else(|| first_evidence_summary(evidence))
            .unwrap_or_else(|| "Review required before any platform write.".to_string()),
        confidence_score: payload_number(payload, "confidence_score")
            .unwrap_or_else(|| f64::from(item.risk_score.min(100)) / 100.0)
            .clamp(0.0, 1.0),
        status: review_item_status(item.status),
        sync_cursor: item.sync_cursor_value,
        updated_at_display: iso8601_utc(item.updated_at),
        ledger_status: item.ledger_status.map(action_status),
        operation_id: item.operation_id.clone(),
    }
}

fn proposed_action_dto(action: &StoredReviewInboxAction) -> ProposedActionDto {
    let payload = Some(&action.suggested_payload);
    ProposedActionDto {
        id: action.id.clone(),
        review_item_id: action.review_item_id.clone(),
        tenant_id: action.tenant_id.clone(),
        actor_user_id: action.actor_user_id.clone(),
        target_user_id: action.target_user_id.clone(),
        owner_user_id: action.owner_user_id.clone(),
        version: action.version,
        status: proposed_action_status(action.status),
        kind: proposed_action_kind(&action.kind),
        risk_severity: risk_severity(action.risk_severity),
        evidence_ids: action.evidence_ids.clone(),
        rationale: whitelisted_payload_string(payload, "rationale")
            .unwrap_or_else(|| format!("Review proposed action {} before execution.", action.id)),
        expected_impact: whitelisted_payload_string(payload, "expected_impact")
            .unwrap_or_else(|| "No production write will occur before confirmation.".to_string()),
        dry_run_result_summary: whitelisted_payload_string(payload, "dry_run_result_summary")
            .unwrap_or_else(|| "Dry-run summary unavailable.".to_string()),
        estimated_write_targets_count: payload_integer(payload, "estimated_write_targets_count")
            .unwrap_or(0),
        decision: action
            .decision
            .as_ref()
            .map(|decision| proposed_action_decision(decision.decision)),
    }
}

fn evidence_dto(
    evidence: &StoredReviewInboxEvidence,
    action_payload: Option<&Value>,
) -> EvidenceItemDto {
    EvidenceItemDto {
        id: evidence.item.id.clone(),
        review_item_id: evidence.review_item_id.clone(),
        source_kind: evidence_source_kind(evidence.item.source_kind),
        source_id: evidence.item.source_id.clone(),
        locator: evidence.item.locator.clone(),
        observed_at_display: iso8601_utc(evidence.item.observed_at),
        summary: sanitized_text_or(Some(evidence.item.summary.as_str()), "Evidence unavailable"),
        signal_type: whitelisted_payload_string(action_payload, "signal_type")
            .as_deref()
            .map(signal_type)
            .unwrap_or_else(|| default_signal_type(evidence.item.source_kind)),
        trust_score: payload_number(action_payload, "trust_score")
            .unwrap_or(0.7)
            .clamp(0.0, 1.0),
        content_hash: evidence.item.content_hash.clone(),
        visibility: evidence_visibility(evidence.item.visibility_scope),
    }
}

fn ledger_event_dto(event: &StoredReviewInboxLedgerEvent) -> LedgerEventDto {
    LedgerEventDto {
        id: event.id.clone(),
        action_id: event.action_id.clone(),
        stage: ledger_stage(event.stage),
        stage_status: ledger_status(event.stage_status),
        timestamp_display: iso8601_utc(event.timestamp),
        message: safe_ledger_text(event.message.as_str(), "Ledger event recorded."),
        idempotency_key: safe_correlation_key(event.idempotency_key.as_str()),
    }
}

fn first_evidence_summary(evidence: Option<&[&StoredReviewInboxEvidence]>) -> Option<String> {
    evidence
        .and_then(|items| items.first())
        .map(|evidence| sanitized_text_or(Some(evidence.item.summary.as_str()), ""))
        .filter(|value| !value.is_empty())
}

fn whitelisted_payload_string(payload: Option<&Value>, key: &str) -> Option<String> {
    payload
        .and_then(|value| value.get(key))
        .and_then(Value::as_str)
        .map(|value| sanitized_text_or(Some(value), ""))
        .filter(|value| !value.is_empty())
}

fn payload_number(payload: Option<&Value>, key: &str) -> Option<f64> {
    payload
        .and_then(|value| value.get(key))
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
}

fn payload_integer(payload: Option<&Value>, key: &str) -> Option<u64> {
    payload
        .and_then(|value| value.get(key))
        .and_then(Value::as_u64)
}

fn sanitized_text_or(value: Option<&str>, fallback: &str) -> String {
    let candidate = value.unwrap_or("").trim();
    let candidate = if candidate.is_empty() {
        fallback.trim()
    } else {
        candidate
    };
    let compact = candidate
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect::<String>();
    let compact = compact.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() > 320 {
        compact.chars().take(320).collect()
    } else {
        compact
    }
}

fn safe_ledger_text(value: &str, fallback: &str) -> String {
    let sanitized = sanitized_text_or(Some(value), fallback);
    if oar_core::security::contains_sensitive_marker(&sanitized) {
        fallback.to_string()
    } else {
        sanitized
    }
}

fn safe_correlation_key(value: &str) -> String {
    let sanitized = sanitized_text_or(Some(value), "redacted");
    let safe_shape = !sanitized.is_empty()
        && sanitized.chars().count() <= 160
        && sanitized
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, ':' | '_' | '-' | '.' | '/'));
    if safe_shape && !oar_core::security::contains_sensitive_marker(&sanitized) {
        sanitized
    } else {
        "redacted".to_string()
    }
}

fn review_item_status(status: ReviewInboxItemStatus) -> &'static str {
    match status {
        ReviewInboxItemStatus::Open => "open",
        ReviewInboxItemStatus::Confirmed => "confirmed",
        ReviewInboxItemStatus::Rejected => "rejected",
        ReviewInboxItemStatus::Executing => "executing",
        ReviewInboxItemStatus::Succeeded => "succeeded",
        ReviewInboxItemStatus::Failed => "failed",
        ReviewInboxItemStatus::Withdrawn => "withdrawn",
    }
}

fn proposed_action_status(status: ProposedActionStatus) -> &'static str {
    match status {
        ProposedActionStatus::Draft => "draft",
        ProposedActionStatus::Published => "published",
        ProposedActionStatus::Superseded => "superseded",
        ProposedActionStatus::Withdrawn => "withdrawn",
    }
}

fn proposed_action_kind(kind: &ProposedActionKind) -> String {
    match kind {
        ProposedActionKind::CreateKrProgress => "create_kr_progress".to_string(),
        ProposedActionKind::UpdateKrProgress => "update_kr_progress".to_string(),
        ProposedActionKind::DeleteKrProgressDryRun => "delete_kr_progress_dry_run".to_string(),
        ProposedActionKind::Custom(custom)
            if matches!(
                custom.as_str(),
                "ping_owner" | "create_task" | "schedule_review"
            ) =>
        {
            custom.clone()
        }
        ProposedActionKind::Custom(_) => "custom".to_string(),
    }
}

fn risk_severity(severity: RiskSeverity) -> &'static str {
    match severity {
        RiskSeverity::Low => "low",
        RiskSeverity::Medium => "medium",
        RiskSeverity::High => "high",
        RiskSeverity::Critical => "critical",
    }
}

fn proposed_action_decision(decision: StoredProposedActionDecisionKind) -> &'static str {
    match decision {
        StoredProposedActionDecisionKind::Confirm => "confirm",
        StoredProposedActionDecisionKind::EditThenConfirm => "edit_then_confirm",
        StoredProposedActionDecisionKind::Reject => "reject",
    }
}

fn action_status(status: ActionStatus) -> &'static str {
    match status {
        ActionStatus::Proposed => "proposed",
        ActionStatus::Confirmed => "confirmed",
        ActionStatus::Executing => "executing",
        ActionStatus::Succeeded => "succeeded",
        ActionStatus::Failed => "failed",
        ActionStatus::Cancelled => "cancelled",
    }
}

fn ledger_stage(stage: StoredReviewInboxLedgerStage) -> &'static str {
    match stage {
        StoredReviewInboxLedgerStage::ConfirmedAction => "confirmed_action",
        StoredReviewInboxLedgerStage::OperationLedger => "operation_ledger",
        StoredReviewInboxLedgerStage::PlatformAdapter => "platform_adapter",
        StoredReviewInboxLedgerStage::AuditEvent => "audit_event",
    }
}

fn ledger_status(status: StoredReviewInboxLedgerStatus) -> &'static str {
    match status {
        StoredReviewInboxLedgerStatus::Pending => "pending",
        StoredReviewInboxLedgerStatus::Ok => "ok",
        StoredReviewInboxLedgerStatus::Error => "error",
    }
}

fn evidence_source_kind(source: EvidenceSourceKind) -> &'static str {
    match source {
        EvidenceSourceKind::OkrProgress => "okr_progress",
        EvidenceSourceKind::LarkMinutes => "lark_minutes",
        EvidenceSourceKind::LarkDoc => "lark_doc",
        EvidenceSourceKind::ManualReviewNote => "manual_review_note",
        EvidenceSourceKind::AuditEvent => "audit_event",
    }
}

fn evidence_visibility(visibility: EvidenceVisibilityScope) -> &'static str {
    match visibility {
        EvidenceVisibilityScope::Tenant => "tenant",
        EvidenceVisibilityScope::Team => "team",
        EvidenceVisibilityScope::User => "user",
    }
}

fn signal_type(value: &str) -> &'static str {
    match value {
        "progress" => "progress",
        "blocker" => "blocker",
        "dependency" => "dependency",
        _ => "cadence",
    }
}

fn default_signal_type(source: EvidenceSourceKind) -> &'static str {
    match source {
        EvidenceSourceKind::OkrProgress => "progress",
        _ => "cadence",
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use hyper::http::{Method, StatusCode};
    use oar_core::action::confirmed_action::ActionStatus;
    use oar_core::domain::evidence::{EvidenceSourceKind, EvidenceVisibilityScope};
    use oar_core::domain::proposed_action::{
        ProposedActionKind, ProposedActionStatus, RiskSeverity,
    };
    use oar_core::domain::review_inbox::ReviewInboxItemStatus;
    use oar_core::storage::postgres::{
        StoredEvidenceItem, StoredProposedActionDecisionKind, StoredReviewInboxAction,
        StoredReviewInboxActionDecision, StoredReviewInboxEvidence, StoredReviewInboxItem,
        StoredReviewInboxLedgerEvent, StoredReviewInboxLedgerStage, StoredReviewInboxLedgerStatus,
        StoredReviewInboxSnapshot,
    };
    use serde_json::{json, Value};

    use super::{decode_review_decision_request, is_body_route, snapshot_response_body};

    #[test]
    fn decision_body_route_predicate_only_matches_post_decisions() {
        assert!(is_body_route(&Method::POST, "/review-inbox/decisions"));
        assert!(!is_body_route(&Method::GET, "/review-inbox/decisions"));
        assert!(!is_body_route(&Method::POST, "/review-inbox/snapshot"));
        assert!(!is_body_route(&Method::POST, "/agent/stream"));
    }

    #[test]
    fn decision_request_decode_rejects_invalid_json_safely() {
        let response =
            decode_review_decision_request(br#"{"action_id":"pa_1","#).expect_err("invalid json");
        let body: Value = serde_json::from_str(&response.body).expect("json");

        assert_eq!(response.status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "review_decision_invalid_json");
        assert!(!response.body.contains("authorization"));
        assert!(!response.body.contains("token"));
    }

    #[test]
    fn decision_request_decode_accepts_swift_contract_body() {
        let request = decode_review_decision_request(
            br#"{
                "action_id":"pa_1",
                "action_version":2,
                "decision":"confirm",
                "note":"ok",
                "expected_sync_cursor":42
            }"#,
        )
        .expect("valid request");

        assert_eq!(request.action_id, "pa_1");
        assert_eq!(request.action_version, 2);
        assert_eq!(request.expected_sync_cursor, Some(42));
    }

    #[test]
    fn mapper_uses_safe_fallbacks_for_missing_display_fields() {
        let snapshot = StoredReviewInboxSnapshot {
            items: vec![StoredReviewInboxItem {
                id: "ri_1".to_string(),
                tenant_id: "tenant_1".to_string(),
                user_id: "user_1".to_string(),
                proposed_action_id: "pa_1".to_string(),
                proposed_action_version: 1,
                risk_score: 72,
                priority: 4,
                status: ReviewInboxItemStatus::Open,
                sort_key: 10,
                sync_cursor_value: 99,
                updated_at: UNIX_EPOCH + Duration::from_secs(60),
                ledger_status: Some(ActionStatus::Confirmed),
                operation_id: Some("op_1".to_string()),
            }],
            actions: Vec::new(),
            evidence: Vec::new(),
            ledger_events: Vec::new(),
        };

        let body = snapshot_response_body(&snapshot, UNIX_EPOCH);
        let item = &body["items"][0];

        assert_eq!(item["objective_title"], "Review item ri_1");
        assert_eq!(item["key_result_title"], "Review required");
        assert_eq!(item["owner_display_name"], "Unassigned");
        assert_eq!(item["week_label"], "Current week");
        assert_eq!(
            item["risk_reason"],
            "Review required before any platform write."
        );
        assert_eq!(item["confidence_score"], 0.72);
        assert_eq!(item["sync_cursor"], 99);
        assert_eq!(item["ledger_status"], "confirmed");
        assert_eq!(item["updated_at_display"], "1970-01-01T00:01:00Z");
    }

    #[test]
    fn mapper_never_returns_raw_suggested_payload() {
        let snapshot = StoredReviewInboxSnapshot {
            items: Vec::new(),
            actions: vec![StoredReviewInboxAction {
                review_item_id: "ri_1".to_string(),
                id: "pa_1".to_string(),
                tenant_id: "tenant_1".to_string(),
                actor_user_id: "actor_1".to_string(),
                target_user_id: None,
                owner_user_id: Some("owner_1".to_string()),
                version: 7,
                status: ProposedActionStatus::Published,
                kind: ProposedActionKind::UpdateKrProgress,
                risk_severity: RiskSeverity::High,
                evidence_ids: vec!["ev_1".to_string()],
                suggested_payload: json!({
                    "rationale": "Safe rationale.",
                    "expected_impact": "Safe impact.",
                    "dry_run_result_summary": "Would update one progress record.",
                    "estimated_write_targets_count": 1,
                    "access_token": "secret-token",
                    "raw_transcript": "full private transcript"
                }),
                decision: None,
            }],
            evidence: Vec::new(),
            ledger_events: Vec::new(),
        };

        let body = snapshot_response_body(&snapshot, UNIX_EPOCH);
        let action = &body["proposed_actions"][0];
        let serialized = serde_json::to_string(&body).expect("json body");

        assert_eq!(action["rationale"], "Safe rationale.");
        assert_eq!(action["expected_impact"], "Safe impact.");
        assert_eq!(
            action["dry_run_result_summary"],
            "Would update one progress record."
        );
        assert_eq!(action["estimated_write_targets_count"], 1);
        assert_eq!(action.get("suggested_payload"), None);
        assert!(!serialized.contains("secret-token"));
        assert!(!serialized.contains("raw_transcript"));
        assert!(!serialized.contains("full private transcript"));
    }

    #[test]
    fn mapper_does_not_leak_edit_then_confirm_payload() {
        let snapshot = StoredReviewInboxSnapshot {
            items: Vec::new(),
            actions: vec![StoredReviewInboxAction {
                review_item_id: "ri_1".to_string(),
                id: "pa_edit".to_string(),
                tenant_id: "tenant_1".to_string(),
                actor_user_id: "actor_1".to_string(),
                target_user_id: None,
                owner_user_id: None,
                version: 1,
                status: ProposedActionStatus::Published,
                kind: ProposedActionKind::Custom("adapter_specific".to_string()),
                risk_severity: RiskSeverity::Medium,
                evidence_ids: vec!["ev_1".to_string()],
                suggested_payload: json!({ "rationale": "Safe rationale." }),
                decision: Some(StoredReviewInboxActionDecision {
                    id: "decision_1".to_string(),
                    actor_user_id: "actor_1".to_string(),
                    decision: StoredProposedActionDecisionKind::EditThenConfirm,
                    confirmed_action_id: Some("ca_1".to_string()),
                    decided_at: UNIX_EPOCH,
                }),
            }],
            evidence: Vec::new(),
            ledger_events: Vec::new(),
        };

        let body = snapshot_response_body(&snapshot, UNIX_EPOCH);
        let serialized = serde_json::to_string(&body).expect("json body");

        assert_eq!(body["proposed_actions"][0]["decision"], "edit_then_confirm");
        assert_eq!(body["proposed_actions"][0]["kind"], "custom");
        assert!(!serialized.contains("edited-secret"));
        assert!(!serialized.contains("access_token"));
    }

    #[test]
    fn mapper_uses_evidence_summary_without_raw_content() {
        let observed = UNIX_EPOCH + Duration::from_secs(86_400);
        let snapshot = StoredReviewInboxSnapshot {
            items: vec![StoredReviewInboxItem {
                id: "ri_1".to_string(),
                tenant_id: "tenant_1".to_string(),
                user_id: "user_1".to_string(),
                proposed_action_id: "pa_1".to_string(),
                proposed_action_version: 1,
                risk_score: 20,
                priority: 1,
                status: ReviewInboxItemStatus::Open,
                sort_key: 1,
                sync_cursor_value: 2,
                updated_at: observed,
                ledger_status: None,
                operation_id: None,
            }],
            actions: vec![StoredReviewInboxAction {
                review_item_id: "ri_1".to_string(),
                id: "pa_1".to_string(),
                tenant_id: "tenant_1".to_string(),
                actor_user_id: "actor_1".to_string(),
                target_user_id: None,
                owner_user_id: None,
                version: 1,
                status: ProposedActionStatus::Published,
                kind: ProposedActionKind::UpdateKrProgress,
                risk_severity: RiskSeverity::Low,
                evidence_ids: vec!["ev_1".to_string()],
                suggested_payload: json!({ "trust_score": 0.92, "signal_type": "blocker" }),
                decision: None,
            }],
            evidence: vec![StoredReviewInboxEvidence {
                review_item_id: "ri_1".to_string(),
                item: StoredEvidenceItem {
                    id: "ev_1".to_string(),
                    tenant_id: "tenant_1".to_string(),
                    summary: "KR has no recent progress.".to_string(),
                    source_kind: EvidenceSourceKind::OkrProgress,
                    source_id: "kr_1".to_string(),
                    locator: Some("okr://kr_1".to_string()),
                    content_hash:
                        "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                            .to_string(),
                    visibility_scope: EvidenceVisibilityScope::Tenant,
                    observed_at: observed,
                    recorded_at: observed,
                },
            }],
            ledger_events: Vec::new(),
        };

        let body = snapshot_response_body(&snapshot, UNIX_EPOCH);

        assert_eq!(
            body["items"][0]["key_result_title"],
            "KR has no recent progress."
        );
        assert_eq!(
            body["items"][0]["risk_reason"],
            "KR has no recent progress."
        );
        assert_eq!(body["evidence"][0]["summary"], "KR has no recent progress.");
        assert_eq!(body["evidence"][0]["signal_type"], "blocker");
        assert_eq!(body["evidence"][0]["trust_score"], 0.92);
        assert_eq!(
            body["evidence"][0]["observed_at_display"],
            "1970-01-02T00:00:00Z"
        );
    }

    #[test]
    fn mapper_projects_safe_ledger_events() {
        let snapshot = StoredReviewInboxSnapshot {
            items: Vec::new(),
            actions: Vec::new(),
            evidence: Vec::new(),
            ledger_events: vec![
                StoredReviewInboxLedgerEvent {
                    id: "ledger_1".to_string(),
                    action_id: "pa_1".to_string(),
                    stage: StoredReviewInboxLedgerStage::OperationLedger,
                    stage_status: StoredReviewInboxLedgerStatus::Ok,
                    timestamp: UNIX_EPOCH + Duration::from_secs(120),
                    message: "Operation ledger confirmed.".to_string(),
                    idempotency_key: "decision:pa_1:v1:confirm".to_string(),
                },
                StoredReviewInboxLedgerEvent {
                    id: "ledger_2".to_string(),
                    action_id: "pa_1".to_string(),
                    stage: StoredReviewInboxLedgerStage::PlatformAdapter,
                    stage_status: StoredReviewInboxLedgerStatus::Error,
                    timestamp: UNIX_EPOCH + Duration::from_secs(121),
                    message: "access_token leaked from adapter".to_string(),
                    idempotency_key: "authorization: bearer abc".to_string(),
                },
            ],
        };

        let body = snapshot_response_body(&snapshot, UNIX_EPOCH);
        let serialized = serde_json::to_string(&body).expect("json body");

        assert_eq!(body["ledger_events"][0]["id"], "ledger_1");
        assert_eq!(body["ledger_events"][0]["action_id"], "pa_1");
        assert_eq!(body["ledger_events"][0]["stage"], "operation_ledger");
        assert_eq!(body["ledger_events"][0]["stage_status"], "ok");
        assert_eq!(
            body["ledger_events"][0]["timestamp_display"],
            "1970-01-01T00:02:00Z"
        );
        assert_eq!(
            body["ledger_events"][0]["message"],
            "Operation ledger confirmed."
        );
        assert_eq!(
            body["ledger_events"][0]["idempotency_key"],
            "decision:pa_1:v1:confirm"
        );
        assert_eq!(body["ledger_events"][1]["stage"], "platform_adapter");
        assert_eq!(body["ledger_events"][1]["stage_status"], "error");
        assert_eq!(
            body["ledger_events"][1]["message"],
            "Ledger event recorded."
        );
        assert_eq!(body["ledger_events"][1]["idempotency_key"], "redacted");
        assert!(!serialized.contains("access_token"));
        assert!(!serialized.contains("bearer abc"));
    }

    #[test]
    fn empty_snapshot_matches_swift_contract_keys() {
        let body = snapshot_response_body(
            &StoredReviewInboxSnapshot {
                items: Vec::new(),
                actions: Vec::new(),
                evidence: Vec::new(),
                ledger_events: Vec::new(),
            },
            UNIX_EPOCH,
        );

        assert_eq!(body["contract_version"], 1);
        assert_eq!(body["generated_at"], "1970-01-01T00:00:00Z");
        assert_eq!(body["items"], Value::Array(Vec::new()));
        assert_eq!(body["proposed_actions"], Value::Array(Vec::new()));
        assert_eq!(body["evidence"], Value::Array(Vec::new()));
        assert_eq!(body["ledger_events"], Value::Array(Vec::new()));
    }
}

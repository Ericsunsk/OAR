use std::sync::Arc;
use std::time::SystemTime;

use http_body_util::{BodyExt, LengthLimitError, Limited};
use hyper::body::Incoming;
use hyper::http::{Method, StatusCode};
use oar_core::storage::postgres::PostgresReviewInboxRepository;
use serde_json::json;

use crate::response::{json_facade_response, not_found, service_unavailable, FacadeResponse};
use crate::runtime::OarHttpFacadeRuntime;
use crate::{authenticate_oar_session, oar_session_auth_error_response, AuthenticatedContext};

mod decision;
mod dto;
mod projection;

use decision::{decode_review_decision_request, record_decision_for_context};
use projection::snapshot_response_body;

const DEFAULT_SNAPSHOT_LIMIT: u32 = 100;
const REVIEW_DECISIONS_PATH: &str = "/review-inbox/decisions";
const REVIEW_DECISION_BODY_LIMIT_BYTES: usize = 64 * 1024;

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
    let Some(persistence) = runtime.persistence() else {
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

    use super::decision::decode_review_decision_request;
    use super::is_body_route;
    use super::projection::snapshot_response_body;

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

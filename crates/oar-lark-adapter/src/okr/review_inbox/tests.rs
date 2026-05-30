use std::collections::HashSet;
use std::time::{Duration, UNIX_EPOCH};

use oar_core::domain::proposed_action::ProposedActionStatus;

use crate::okr::types::{OkrReadKeyResult, OkrReadObjective, OkrReadOkr, OkrReadSnapshot};

use super::*;

fn sample_input() -> OkrReviewInboxPlanInput<'static> {
    OkrReviewInboxPlanInput {
        tenant_id: "tenant_1",
        review_user_id: "user_1",
        actor_user_id: "user_1",
        source_cursor: 42,
        observed_at: UNIX_EPOCH + Duration::from_secs(1_000),
        recorded_at: UNIX_EPOCH + Duration::from_secs(2_000),
    }
}

fn sample_snapshot() -> OkrReadSnapshot {
    OkrReadSnapshot {
        okrs: vec![OkrReadOkr {
            okr_id: Some("okr_1".to_string()),
            period_id: Some("period_q2".to_string()),
            okr_name: Some("Q2".to_string()),
            objectives: vec![OkrReadObjective {
                objective_id: Some("obj_1".to_string()),
                content: Some(
                    "sensitive objective text that should never be copied into payload".to_string(),
                ),
                progress: Some("20".to_string()),
                status: Some("1".to_string()),
                progress_record_ids: vec!["opr_1".to_string()],
                deadline: Some("1782777600000".to_string()),
                last_updated_time: Some("1780000000000".to_string()),
                krs: vec![
                    OkrReadKeyResult {
                        kr_id: Some("kr_1".to_string()),
                        content: Some(
                            "very sensitive kr text that is deliberately long".to_string(),
                        ),
                        progress: Some("20".to_string()),
                        status: Some("1".to_string()),
                        progress_record_ids: vec!["kpr_1".to_string()],
                        deadline: Some("1782777600000".to_string()),
                        last_updated_time: Some("1780000000000".to_string()),
                    },
                    OkrReadKeyResult {
                        kr_id: Some("kr_healthy".to_string()),
                        content: Some("healthy kr".to_string()),
                        progress: Some("90".to_string()),
                        status: Some("0".to_string()),
                        progress_record_ids: vec![],
                        deadline: None,
                        last_updated_time: None,
                    },
                ],
            }],
        }],
    }
}

#[test]
fn planner_builds_stable_evidence_actions_and_inbox_items() {
    let first = plan_okr_review_inbox_sync(sample_input(), &sample_snapshot()).expect("plan");
    let second = plan_okr_review_inbox_sync(sample_input(), &sample_snapshot()).expect("plan");

    assert_eq!(first, second);
    assert_eq!(first.evidence_items.len(), 2);
    assert_eq!(first.proposed_actions.len(), 1);
    assert_eq!(first.inbox_items.len(), 1);
    assert_eq!(
        first.proposed_actions[0].status,
        ProposedActionStatus::Published
    );
    assert_eq!(first.proposed_actions[0].version, 42);
    assert_eq!(first.proposed_actions[0].evidence_ids.len(), 1);
    assert_eq!(
        first.inbox_items[0].proposed_action_id,
        first.proposed_actions[0].id.0
    );
    assert_eq!(first.inbox_items[0].sync_cursor, 42);
    assert!(first.evidence_items[0].content_hash.starts_with("sha256:"));
    assert_eq!(
        first.evidence_items[0].reference.source_id,
        "okr:okr_1:objective:obj_1:kr:kr_1"
    );
    assert_eq!(
        first.evidence_items[0].reference.locator.as_deref(),
        Some("okr://okr_1/objectives/obj_1/krs/kr_1")
    );
}

#[test]
fn planner_skips_records_without_stable_kr_identity() {
    let snapshot = OkrReadSnapshot {
        okrs: vec![OkrReadOkr {
            okr_id: Some("okr_1".to_string()),
            period_id: None,
            okr_name: None,
            objectives: vec![OkrReadObjective {
                objective_id: Some("obj_1".to_string()),
                content: None,
                progress: None,
                status: None,
                progress_record_ids: vec![],
                deadline: None,
                last_updated_time: None,
                krs: vec![OkrReadKeyResult {
                    kr_id: None,
                    content: Some("missing id".to_string()),
                    progress: None,
                    status: None,
                    progress_record_ids: vec![],
                    deadline: None,
                    last_updated_time: None,
                }],
            }],
        }],
    };

    let plan = plan_okr_review_inbox_sync(sample_input(), &snapshot).expect("plan");
    assert!(plan.evidence_items.is_empty());
    assert!(plan.proposed_actions.is_empty());
    assert!(plan.inbox_items.is_empty());
}

#[test]
fn planner_does_not_copy_full_raw_content_into_summary_or_payload() {
    let plan = plan_okr_review_inbox_sync(sample_input(), &sample_snapshot()).expect("plan");
    let payload = plan.proposed_actions[0].suggested_payload.to_string();

    assert!(!plan.evidence_items[0]
        .summary
        .contains("sensitive objective"));
    assert!(!plan.evidence_items[0].summary.contains("very sensitive kr"));
    assert!(!payload.contains("sensitive objective"));
    assert!(!payload.contains("very sensitive kr"));
}

#[test]
fn planner_digest_ids_do_not_collapse_similar_raw_ids() {
    let snapshot = OkrReadSnapshot {
        okrs: vec![OkrReadOkr {
            okr_id: Some("okr_1".to_string()),
            period_id: None,
            okr_name: None,
            objectives: vec![OkrReadObjective {
                objective_id: Some("obj_1".to_string()),
                content: None,
                progress: None,
                status: None,
                progress_record_ids: vec![],
                deadline: None,
                last_updated_time: None,
                krs: vec![
                    OkrReadKeyResult {
                        kr_id: Some("kr:a".to_string()),
                        content: None,
                        progress: Some("20".to_string()),
                        status: Some("1".to_string()),
                        progress_record_ids: vec![],
                        deadline: None,
                        last_updated_time: None,
                    },
                    OkrReadKeyResult {
                        kr_id: Some("kr/a".to_string()),
                        content: None,
                        progress: Some("20".to_string()),
                        status: Some("1".to_string()),
                        progress_record_ids: vec![],
                        deadline: None,
                        last_updated_time: None,
                    },
                ],
            }],
        }],
    };
    let plan = plan_okr_review_inbox_sync(sample_input(), &snapshot).expect("plan");
    let action_ids = plan
        .proposed_actions
        .iter()
        .map(|action| action.id.0.as_str())
        .collect::<HashSet<_>>();
    let evidence_ids = plan
        .evidence_items
        .iter()
        .map(|item| item.id.0.as_str())
        .collect::<HashSet<_>>();

    assert_eq!(plan.proposed_actions.len(), 2);
    assert_eq!(action_ids.len(), 2);
    assert_eq!(evidence_ids.len(), 2);
}

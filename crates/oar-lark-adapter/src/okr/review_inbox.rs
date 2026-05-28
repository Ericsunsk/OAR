use std::time::SystemTime;

use oar_core::domain::evidence::{
    EvidenceError, EvidenceId, EvidenceItem, EvidenceRef, EvidenceSourceKind,
    EvidenceVisibilityScope,
};
use oar_core::domain::identity::{TenantId, WorkspaceUserId};
use oar_core::domain::proposed_action::{
    ProposedAction, ProposedActionError, ProposedActionId, ProposedActionKind, RiskSeverity,
};
use oar_core::domain::review_inbox::{ReviewInboxItem, ReviewInboxItemId};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::types::{OkrReadKeyResult, OkrReadObjective, OkrReadOkr, OkrReadSnapshot};

#[derive(Debug, Clone, PartialEq)]
pub struct OkrReviewInboxPlan {
    pub evidence_items: Vec<EvidenceItem>,
    pub proposed_actions: Vec<ProposedAction>,
    pub inbox_items: Vec<ReviewInboxItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OkrReviewInboxPlanInput<'a> {
    pub tenant_id: &'a str,
    pub review_user_id: &'a str,
    pub actor_user_id: &'a str,
    pub source_cursor: u64,
    pub observed_at: SystemTime,
    pub recorded_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OkrReviewInboxPlanError {
    Evidence(EvidenceError),
    ProposedAction(ProposedActionError),
}

impl From<EvidenceError> for OkrReviewInboxPlanError {
    fn from(value: EvidenceError) -> Self {
        Self::Evidence(value)
    }
}

impl From<ProposedActionError> for OkrReviewInboxPlanError {
    fn from(value: ProposedActionError) -> Self {
        Self::ProposedAction(value)
    }
}

pub fn plan_okr_review_inbox_sync(
    input: OkrReviewInboxPlanInput<'_>,
    snapshot: &OkrReadSnapshot,
) -> Result<OkrReviewInboxPlan, OkrReviewInboxPlanError> {
    let mut evidence_items = Vec::new();
    let mut proposed_actions = Vec::new();
    let mut inbox_items = Vec::new();
    let version = input.source_cursor.max(1);

    for okr in &snapshot.okrs {
        let Some(okr_id) = okr.okr_id.as_deref().filter(|value| !value.is_empty()) else {
            continue;
        };

        for objective in &okr.objectives {
            let Some(objective_id) = objective
                .objective_id
                .as_deref()
                .filter(|value| !value.is_empty())
            else {
                continue;
            };

            for kr in &objective.krs {
                let Some(kr_id) = kr.kr_id.as_deref().filter(|value| !value.is_empty()) else {
                    continue;
                };

                let source_id = source_id(okr_id, objective_id, kr_id);
                let evidence = build_evidence_item(&input, okr, objective, kr, &source_id)?;
                let evidence_id = evidence.id.0.clone();
                let risk_score = risk_score_for_kr(kr);
                evidence_items.push(evidence);

                if risk_score < 50 {
                    continue;
                }

                let action = build_proposed_action(
                    &input,
                    okr,
                    objective,
                    kr,
                    version,
                    risk_score,
                    evidence_id.clone(),
                )?;
                let inbox_item = build_inbox_item(&input, &action, version, risk_score);
                proposed_actions.push(action);
                inbox_items.push(inbox_item);
            }
        }
    }

    Ok(OkrReviewInboxPlan {
        evidence_items,
        proposed_actions,
        inbox_items,
    })
}

fn build_evidence_item(
    input: &OkrReviewInboxPlanInput<'_>,
    okr: &OkrReadOkr,
    objective: &OkrReadObjective,
    kr: &OkrReadKeyResult,
    source_id: &str,
) -> Result<EvidenceItem, OkrReviewInboxPlanError> {
    let reference = EvidenceRef::new(
        EvidenceSourceKind::OkrProgress,
        source_id,
        Some(locator_for_kr(okr, objective, kr)),
    )?;
    let content_hash = hash_evidence(okr, objective, kr);
    EvidenceItem::new(
        EvidenceId(format!(
            "evidence:okr-progress:{}",
            stable_id_digest(source_id)
        )),
        kr_summary(kr),
        reference,
        content_hash,
        EvidenceVisibilityScope::User,
        input.observed_at,
        input.recorded_at,
    )
    .map_err(Into::into)
}

fn build_proposed_action(
    input: &OkrReviewInboxPlanInput<'_>,
    okr: &OkrReadOkr,
    objective: &OkrReadObjective,
    kr: &OkrReadKeyResult,
    version: u64,
    risk_score: u32,
    evidence_id: String,
) -> Result<ProposedAction, OkrReviewInboxPlanError> {
    let mut action = ProposedAction::draft(
        ProposedActionId(format!(
            "pa:okr-progress:{}",
            stable_id_digest(&source_id(
                okr.okr_id.as_deref().unwrap_or_default(),
                objective.objective_id.as_deref().unwrap_or_default(),
                kr.kr_id.as_deref().unwrap_or_default(),
            ))
        )),
        TenantId(input.tenant_id.to_string()),
        WorkspaceUserId(input.actor_user_id.to_string()),
        Some(WorkspaceUserId(input.review_user_id.to_string())),
        Some(WorkspaceUserId(input.review_user_id.to_string())),
        version,
        ProposedActionKind::UpdateKrProgress,
        risk_severity_for_score(risk_score),
        vec![evidence_id],
        suggested_payload(okr, objective, kr, risk_score),
    )?;
    action.publish()?;
    Ok(action)
}

fn build_inbox_item(
    input: &OkrReviewInboxPlanInput<'_>,
    action: &ProposedAction,
    version: u64,
    risk_score: u32,
) -> ReviewInboxItem {
    ReviewInboxItem::new(
        ReviewInboxItemId(format!(
            "inbox:okr-progress:{}",
            stable_id_digest(&action.id.0)
        )),
        TenantId(input.tenant_id.to_string()),
        WorkspaceUserId(input.review_user_id.to_string()),
        action.id.0.clone(),
        version,
        risk_score,
        risk_score,
        sort_key(input.source_cursor, risk_score),
        input.source_cursor,
        input.recorded_at,
    )
}

fn suggested_payload(
    okr: &OkrReadOkr,
    objective: &OkrReadObjective,
    kr: &OkrReadKeyResult,
    risk_score: u32,
) -> Value {
    json!({
        "action": "update_kr_progress",
        "target": {
            "okr_id": okr.okr_id.as_deref(),
            "objective_id": objective.objective_id.as_deref(),
            "kr_id": kr.kr_id.as_deref(),
        },
        "observed": {
            "progress_percent": kr.progress.as_deref(),
            "progress_status": kr.status.as_deref(),
            "deadline": kr.deadline.as_deref(),
            "last_updated_time": kr.last_updated_time.as_deref(),
            "progress_record_ids": &kr.progress_record_ids,
        },
        "risk_score": risk_score,
    })
}

fn hash_evidence(okr: &OkrReadOkr, objective: &OkrReadObjective, kr: &OkrReadKeyResult) -> String {
    let canonical = json!({
        "okr_id": okr.okr_id.as_deref(),
        "objective_id": objective.objective_id.as_deref(),
        "kr_id": kr.kr_id.as_deref(),
        "objective_content": objective.content.as_deref(),
        "kr_content": kr.content.as_deref(),
        "progress": kr.progress.as_deref(),
        "status": kr.status.as_deref(),
        "deadline": kr.deadline.as_deref(),
        "last_updated_time": kr.last_updated_time.as_deref(),
        "progress_record_ids": &kr.progress_record_ids,
    });
    let bytes = serde_json::to_vec(&canonical).unwrap_or_default();
    let digest = Sha256::digest(bytes);
    format!("sha256:{}", hex::encode(digest))
}

fn kr_summary(kr: &OkrReadKeyResult) -> String {
    let kr_id = kr.kr_id.as_deref().unwrap_or("unknown");
    let progress = kr.progress.as_deref().unwrap_or("unknown");
    let status = kr.status.as_deref().unwrap_or("unknown");
    format!("KR {kr_id} progress {progress}, status {status}")
}

fn risk_score_for_kr(kr: &OkrReadKeyResult) -> u32 {
    let status_score = match kr.status.as_deref() {
        Some("2") | Some("delayed") | Some("delay") => 85,
        Some("1") | Some("risk") => 70,
        Some("-1") | None => 55,
        Some("0") | Some("normal") => 20,
        Some(_) => 45,
    };
    let progress_score = match parse_progress(kr.progress.as_deref()) {
        Some(value) if value < 30.0 => 75,
        Some(value) if value < 50.0 => 60,
        Some(value) if value < 70.0 => 45,
        Some(_) => 20,
        None => 55,
    };
    status_score.max(progress_score)
}

fn risk_severity_for_score(score: u32) -> RiskSeverity {
    match score {
        85..=u32::MAX => RiskSeverity::High,
        60..=84 => RiskSeverity::Medium,
        _ => RiskSeverity::Low,
    }
}

fn parse_progress(value: Option<&str>) -> Option<f64> {
    value?.trim().parse::<f64>().ok()
}

fn sort_key(source_cursor: u64, risk_score: u32) -> i64 {
    let cursor = source_cursor.min(999_999) as i64;
    i64::from(risk_score) * 1_000_000 + cursor
}

fn source_id(okr_id: &str, objective_id: &str, kr_id: &str) -> String {
    format!("okr:{okr_id}:objective:{objective_id}:kr:{kr_id}")
}

fn locator_for_kr(okr: &OkrReadOkr, objective: &OkrReadObjective, kr: &OkrReadKeyResult) -> String {
    format!(
        "okr://{}/objectives/{}/krs/{}",
        okr.okr_id.as_deref().unwrap_or_default(),
        objective.objective_id.as_deref().unwrap_or_default(),
        kr.kr_id.as_deref().unwrap_or_default()
    )
}

fn stable_id_digest(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::time::{Duration, UNIX_EPOCH};

    use oar_core::domain::proposed_action::ProposedActionStatus;

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
                        "sensitive objective text that should never be copied into payload"
                            .to_string(),
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
}

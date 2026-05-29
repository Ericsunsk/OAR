use std::time::SystemTime;

use oar_core::domain::evidence::{EvidenceError, EvidenceItem};
use oar_core::domain::proposed_action::{ProposedAction, ProposedActionError};
use oar_core::domain::review_inbox::ReviewInboxItem;

use super::types::{OkrReadKeyResult, OkrReadObjective, OkrReadOkr, OkrReadSnapshot};
use projection::{build_evidence_item, build_inbox_item, build_proposed_action};
use risk::risk_score_for_kr;

mod projection;
mod risk;

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
        for objective in &okr.objectives {
            for kr in &objective.krs {
                let Some(target) = OkrKrReviewTarget::from_parts(okr, objective, kr) else {
                    continue;
                };

                let evidence = build_evidence_item(&input, &target)?;
                let evidence_id = evidence.id.0.clone();
                let risk_score = risk_score_for_kr(target.kr);
                evidence_items.push(evidence);

                if risk_score < 50 {
                    continue;
                }

                let action = build_proposed_action(
                    &input,
                    &target,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct OkrKrReviewTarget<'a> {
    identity: OkrKrIdentity<'a>,
    okr: &'a OkrReadOkr,
    objective: &'a OkrReadObjective,
    kr: &'a OkrReadKeyResult,
}

impl<'a> OkrKrReviewTarget<'a> {
    fn from_parts(
        okr: &'a OkrReadOkr,
        objective: &'a OkrReadObjective,
        kr: &'a OkrReadKeyResult,
    ) -> Option<Self> {
        Some(Self {
            identity: OkrKrIdentity::from_parts(okr, objective, kr)?,
            okr,
            objective,
            kr,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OkrKrIdentity<'a> {
    okr_id: &'a str,
    objective_id: &'a str,
    kr_id: &'a str,
}

impl<'a> OkrKrIdentity<'a> {
    fn from_parts(
        okr: &'a OkrReadOkr,
        objective: &'a OkrReadObjective,
        kr: &'a OkrReadKeyResult,
    ) -> Option<Self> {
        Some(Self {
            okr_id: non_empty(okr.okr_id.as_deref())?,
            objective_id: non_empty(objective.objective_id.as_deref())?,
            kr_id: non_empty(kr.kr_id.as_deref())?,
        })
    }

    fn source_id(self) -> String {
        format!(
            "okr:{}:objective:{}:kr:{}",
            self.okr_id, self.objective_id, self.kr_id
        )
    }

    fn locator(self) -> String {
        format!(
            "okr://{}/objectives/{}/krs/{}",
            self.okr_id, self.objective_id, self.kr_id
        )
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.is_empty())
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

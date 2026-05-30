use std::time::SystemTime;

use oar_core::domain::evidence::{EvidenceError, EvidenceItem};
use oar_core::domain::proposed_action::{ProposedAction, ProposedActionError};
use oar_core::domain::review_inbox::ReviewInboxItem;

use super::types::OkrReadSnapshot;
use projection::{build_evidence_item, build_inbox_item, build_proposed_action};
use risk::risk_score_for_kr;
use target::OkrKrReviewTarget;

mod projection;
mod risk;
mod target;

#[cfg(test)]
mod tests;

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

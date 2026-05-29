use oar_core::domain::evidence::{
    EvidenceId, EvidenceItem, EvidenceRef, EvidenceSourceKind, EvidenceVisibilityScope,
};
use oar_core::domain::identity::{TenantId, WorkspaceUserId};
use oar_core::domain::proposed_action::{ProposedAction, ProposedActionId, ProposedActionKind};
use oar_core::domain::review_inbox::{ReviewInboxItem, ReviewInboxItemId};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::okr::types::{OkrReadKeyResult, OkrReadObjective, OkrReadOkr};

use super::risk::risk_severity_for_score;
use super::{OkrKrReviewTarget, OkrReviewInboxPlanError, OkrReviewInboxPlanInput};

pub(super) fn build_evidence_item(
    input: &OkrReviewInboxPlanInput<'_>,
    target: &OkrKrReviewTarget<'_>,
) -> Result<EvidenceItem, OkrReviewInboxPlanError> {
    let source_id = target.identity.source_id();
    let reference = EvidenceRef::new(
        EvidenceSourceKind::OkrProgress,
        &source_id,
        Some(target.identity.locator()),
    )?;
    let content_hash = hash_evidence(target.okr, target.objective, target.kr);
    EvidenceItem::new(
        EvidenceId(format!(
            "evidence:okr-progress:{}",
            stable_id_digest(&source_id)
        )),
        kr_summary(target.kr),
        reference,
        content_hash,
        EvidenceVisibilityScope::User,
        input.observed_at,
        input.recorded_at,
    )
    .map_err(Into::into)
}

pub(super) fn build_proposed_action(
    input: &OkrReviewInboxPlanInput<'_>,
    target: &OkrKrReviewTarget<'_>,
    version: u64,
    risk_score: u32,
    evidence_id: String,
) -> Result<ProposedAction, OkrReviewInboxPlanError> {
    let mut action = ProposedAction::draft(
        ProposedActionId(format!(
            "pa:okr-progress:{}",
            stable_id_digest(&target.identity.source_id())
        )),
        TenantId(input.tenant_id.to_string()),
        WorkspaceUserId(input.actor_user_id.to_string()),
        Some(WorkspaceUserId(input.review_user_id.to_string())),
        Some(WorkspaceUserId(input.review_user_id.to_string())),
        version,
        ProposedActionKind::UpdateKrProgress,
        risk_severity_for_score(risk_score),
        vec![evidence_id],
        suggested_payload(target.okr, target.objective, target.kr, risk_score),
    )?;
    action.publish()?;
    Ok(action)
}

pub(super) fn build_inbox_item(
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

fn sort_key(source_cursor: u64, risk_score: u32) -> i64 {
    let cursor = source_cursor.min(999_999) as i64;
    i64::from(risk_score) * 1_000_000 + cursor
}

fn stable_id_digest(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

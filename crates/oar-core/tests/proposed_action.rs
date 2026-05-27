use std::time::SystemTime;

use serde_json::json;

use oar_core::action::confirmed_action::ActionStatus;
use oar_core::domain::identity::{OarUserId, TenantId};
use oar_core::domain::proposed_action::{
    ProposedAction, ProposedActionDecision, ProposedActionError, ProposedActionId,
    ProposedActionKind, ProposedActionStatus, RiskSeverity,
};

fn draft_action(version: u64) -> ProposedAction {
    ProposedAction::draft(
        ProposedActionId("pa-1".to_string()),
        TenantId("tenant-1".to_string()),
        OarUserId("actor-1".to_string()),
        Some(OarUserId("target-1".to_string())),
        Some(OarUserId("owner-1".to_string())),
        version,
        ProposedActionKind::UpdateKrProgress,
        RiskSeverity::High,
        vec!["evidence-1".to_string()],
        json!({
            "okr_id": "okr-1",
            "kr_id": "kr-1",
            "suggested_progress": 70
        }),
    )
    .expect("draft action should be valid")
}

#[test]
fn draft_cannot_confirm() {
    let mut action = draft_action(1);

    let result = action.decide(ProposedActionDecision::Confirm, SystemTime::UNIX_EPOCH);

    assert_eq!(
        result,
        Err(ProposedActionError::InvalidStatusForDecision {
            status: ProposedActionStatus::Draft
        })
    );
}

#[test]
fn no_evidence_cannot_publish_or_create() {
    let create_result = ProposedAction::draft(
        ProposedActionId("pa-2".to_string()),
        TenantId("tenant-1".to_string()),
        OarUserId("actor-1".to_string()),
        None,
        None,
        1,
        ProposedActionKind::CreateKrProgress,
        RiskSeverity::Medium,
        vec![],
        json!({"kr_id": "kr-2"}),
    );
    assert_eq!(create_result, Err(ProposedActionError::EmptyEvidence));

    let mut action = draft_action(1);
    action.evidence_ids.clear();
    assert_eq!(action.publish(), Err(ProposedActionError::EmptyEvidence));
}

#[test]
fn blank_evidence_id_is_rejected() {
    let result = ProposedAction::draft(
        ProposedActionId("pa-blank-evidence".to_string()),
        TenantId("tenant-1".to_string()),
        OarUserId("actor-1".to_string()),
        None,
        None,
        1,
        ProposedActionKind::CreateKrProgress,
        RiskSeverity::Low,
        vec!["   ".to_string()],
        json!({"kr_id": "kr-blank"}),
    );

    assert_eq!(result, Err(ProposedActionError::InvalidEvidenceId));
}

#[test]
fn duplicate_evidence_ids_are_normalized() {
    let action = ProposedAction::draft(
        ProposedActionId("pa-dedup-evidence".to_string()),
        TenantId("tenant-1".to_string()),
        OarUserId("actor-1".to_string()),
        None,
        None,
        1,
        ProposedActionKind::UpdateKrProgress,
        RiskSeverity::Medium,
        vec![
            " evidence-1 ".to_string(),
            "evidence-1".to_string(),
            "evidence-2".to_string(),
        ],
        json!({"kr_id": "kr-dedup"}),
    )
    .expect("draft action should be valid");

    assert_eq!(
        action.evidence_ids,
        vec!["evidence-1".to_string(), "evidence-2".to_string()]
    );
}

#[test]
fn confirm_generates_confirmed_action_with_confirmed_status() {
    let mut action = draft_action(1);
    action.publish().expect("publish should work");

    let confirmed = action
        .decide(ProposedActionDecision::Confirm, SystemTime::UNIX_EPOCH)
        .expect("confirm decision should work")
        .expect("confirm should emit confirmed action");

    assert_eq!(confirmed.status, ActionStatus::Confirmed);
    assert!(confirmed.action_id.contains("pa-1:v1"));
    assert!(confirmed.idempotency_key.contains("pa-1:v1"));
}

#[test]
fn reject_does_not_generate_confirmed_action() {
    let mut action = draft_action(1);
    action.publish().expect("publish should work");

    let confirmed = action
        .decide(ProposedActionDecision::Reject, SystemTime::UNIX_EPOCH)
        .expect("reject decision should work");

    assert!(confirmed.is_none());
}

#[test]
fn edit_then_confirm_has_distinct_idempotency_key_and_version_chain() {
    let mut confirm_action = draft_action(2);
    confirm_action.publish().expect("publish should work");
    let confirmed_from_confirm = confirm_action
        .decide(ProposedActionDecision::Confirm, SystemTime::UNIX_EPOCH)
        .expect("confirm should succeed")
        .expect("confirm should produce action");

    let mut edit_action = draft_action(2);
    edit_action.publish().expect("publish should work");
    let confirmed_from_edit = edit_action
        .decide(
            ProposedActionDecision::EditThenConfirm {
                edited_payload: json!({
                    "okr_id": "okr-1",
                    "kr_id": "kr-1",
                    "suggested_progress": 75
                }),
            },
            SystemTime::UNIX_EPOCH,
        )
        .expect("edit_then_confirm should succeed")
        .expect("edit_then_confirm should produce action");

    assert_ne!(
        confirmed_from_confirm.idempotency_key,
        confirmed_from_edit.idempotency_key
    );
    assert!(confirmed_from_confirm.idempotency_key.contains("pa-1:v2"));
    assert!(confirmed_from_edit.idempotency_key.contains("pa-1:v2"));
}

#[test]
fn idempotency_key_is_stable_and_tenant_scoped() {
    let mut first = draft_action(3);
    first.publish().expect("publish should work");
    let confirmed_first = first
        .decide(ProposedActionDecision::Confirm, SystemTime::UNIX_EPOCH)
        .expect("confirm should succeed")
        .expect("confirm should produce action");

    let mut second = draft_action(3);
    second.publish().expect("publish should work");
    let confirmed_second = second
        .decide(ProposedActionDecision::Confirm, SystemTime::UNIX_EPOCH)
        .expect("confirm should succeed")
        .expect("confirm should produce action");

    assert_eq!(
        confirmed_first.idempotency_key,
        confirmed_second.idempotency_key
    );
    assert!(confirmed_first.idempotency_key.contains("tenant:tenant-1"));

    let mut other_tenant = ProposedAction::draft(
        ProposedActionId("pa-1".to_string()),
        TenantId("tenant-2".to_string()),
        OarUserId("actor-1".to_string()),
        Some(OarUserId("target-1".to_string())),
        Some(OarUserId("owner-1".to_string())),
        3,
        ProposedActionKind::UpdateKrProgress,
        RiskSeverity::High,
        vec!["evidence-1".to_string()],
        json!({
            "okr_id": "okr-1",
            "kr_id": "kr-1",
            "suggested_progress": 70
        }),
    )
    .expect("draft action should be valid");
    other_tenant.publish().expect("publish should work");
    let confirmed_other_tenant = other_tenant
        .decide(ProposedActionDecision::Confirm, SystemTime::UNIX_EPOCH)
        .expect("confirm should succeed")
        .expect("confirm should produce action");

    assert_ne!(
        confirmed_first.idempotency_key,
        confirmed_other_tenant.idempotency_key
    );
    assert!(confirmed_other_tenant
        .idempotency_key
        .contains("tenant:tenant-2"));
}

#[test]
fn published_action_can_be_superseded_before_decision() {
    let mut action = draft_action(1);
    action.publish().expect("publish should work");

    action.supersede().expect("supersede should work");

    assert_eq!(action.status, ProposedActionStatus::Superseded);
    assert_eq!(
        action.decide(ProposedActionDecision::Confirm, SystemTime::UNIX_EPOCH),
        Err(ProposedActionError::InvalidStatusForDecision {
            status: ProposedActionStatus::Superseded
        })
    );
}

#[test]
fn decided_action_cannot_be_withdrawn() {
    let mut action = draft_action(1);
    action.publish().expect("publish should work");
    action
        .decide(ProposedActionDecision::Reject, SystemTime::UNIX_EPOCH)
        .expect("reject should work");

    assert_eq!(
        action.withdraw(),
        Err(ProposedActionError::DecisionAlreadyFinalized)
    );
}

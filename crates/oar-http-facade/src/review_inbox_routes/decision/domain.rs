use std::time::SystemTime;

use oar_core::action::confirmed_action::ConfirmedAction;
use oar_core::domain::identity::{TenantId, WorkspaceUserId};
use oar_core::domain::proposed_action::{ProposedAction, ProposedActionId, ProposedActionKind};
use oar_core::domain::review_inbox::{ReviewInboxItem, ReviewInboxItemId};
use oar_core::storage::postgres::{StoredReviewInboxAction, StoredReviewInboxItem};

use super::super::labels::action_status;

pub(super) fn proposed_action_from_stored(
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

pub(super) fn review_inbox_item_from_stored(
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

pub(super) fn operation_id(action: &ConfirmedAction) -> String {
    format!("op-{}", action.idempotency_key)
}

pub(super) fn is_confirmable_action_kind(kind: &ProposedActionKind) -> bool {
    matches!(kind, ProposedActionKind::UpdateKrProgress)
}

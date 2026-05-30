use serde_json::Value;

use super::confirmed_action::ConfirmedAction;
use crate::domain::proposed_action::ProposedActionKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmedExecutionDecision {
    Confirm,
    EditThenConfirm,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConfirmedExecutionRequest {
    pub confirmed_action: ConfirmedAction,
    pub proposed_action_id: String,
    pub proposed_action_version: u64,
    pub action_kind: ProposedActionKind,
    pub target_user_id: Option<String>,
    pub owner_user_id: Option<String>,
    pub evidence_ids: Vec<String>,
    pub effective_payload: Value,
    pub decision: ConfirmedExecutionDecision,
}

impl ConfirmedExecutionRequest {
    pub fn action(&self) -> &ConfirmedAction {
        &self.confirmed_action
    }
}

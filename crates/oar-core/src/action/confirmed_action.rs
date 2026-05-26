use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionStatus {
    Proposed,
    Confirmed,
    Executing,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedAction {
    pub action_id: String,
    pub tenant_id: String,
    pub actor_user_id: String,
    pub idempotency_key: String,
    pub status: ActionStatus,
    pub confirmed_at: Option<SystemTime>,
}

impl ConfirmedAction {
    pub fn proposed(
        action_id: impl Into<String>,
        tenant_id: impl Into<String>,
        actor_user_id: impl Into<String>,
        idempotency_key: impl Into<String>,
    ) -> Self {
        Self {
            action_id: action_id.into(),
            tenant_id: tenant_id.into(),
            actor_user_id: actor_user_id.into(),
            idempotency_key: idempotency_key.into(),
            status: ActionStatus::Proposed,
            confirmed_at: None,
        }
    }

    pub fn confirm(mut self, at: SystemTime) -> Self {
        self.status = ActionStatus::Confirmed;
        self.confirmed_at = Some(at);
        self
    }
}

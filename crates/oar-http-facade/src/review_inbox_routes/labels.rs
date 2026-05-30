use oar_core::action::confirmed_action::ActionStatus;

use super::dto::ReviewDecisionKindDto;

pub(super) fn action_status(status: ActionStatus) -> &'static str {
    match status {
        ActionStatus::Proposed => "proposed",
        ActionStatus::Confirmed => "confirmed",
        ActionStatus::Executing => "executing",
        ActionStatus::Succeeded => "succeeded",
        ActionStatus::Failed => "failed",
        ActionStatus::Cancelled => "cancelled",
    }
}

pub(super) fn review_decision_kind(decision: ReviewDecisionKindDto) -> &'static str {
    match decision {
        ReviewDecisionKindDto::Confirm => "confirm",
        ReviewDecisionKindDto::EditThenConfirm => "edit_then_confirm",
        ReviewDecisionKindDto::Reject => "reject",
    }
}

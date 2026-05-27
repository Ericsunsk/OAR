use crate::action::confirmed_action::ActionStatus;
use crate::action::execution_policy::ExecutionDenied;

pub(crate) fn safe_denial_message(denial: &ExecutionDenied) -> String {
    match denial {
        ExecutionDenied::TenantMismatch { .. } => {
            "Execution denied by policy: action and token grant belong to different tenants"
                .to_string()
        }
        ExecutionDenied::ActionNotConfirmed { status } => {
            format!("Execution denied by policy: action status is {status:?}, not Confirmed")
        }
        ExecutionDenied::ActorUserMismatch { .. } => {
            "Execution denied by policy: action actor does not match bound actor".to_string()
        }
        ExecutionDenied::IdentityMismatch { .. } => {
            "Execution denied by policy: action actor is not authorized for token grant identity"
                .to_string()
        }
        ExecutionDenied::ActionNotAllowlisted { action_type } => {
            format!("Execution denied by policy: action type {action_type} is not allowlisted")
        }
        ExecutionDenied::ActorKindNotAllowed { actor_kind } => {
            format!("Execution denied by policy: actor kind {actor_kind:?} is not allowed")
        }
        ExecutionDenied::GrantNotExecutable { state } => {
            format!("Execution denied by policy: token grant state {state:?} is not executable")
        }
        ExecutionDenied::MissingScope { required_scope } => {
            format!("Execution denied by policy: missing required scope {required_scope}")
        }
    }
}

pub(crate) fn is_terminal_status(status: ActionStatus) -> bool {
    matches!(
        status,
        ActionStatus::Succeeded | ActionStatus::Failed | ActionStatus::Cancelled
    )
}

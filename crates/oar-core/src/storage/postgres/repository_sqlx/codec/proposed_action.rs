use crate::domain::proposed_action::{
    ProposedActionDecision, ProposedActionKind, ProposedActionStatus, RiskSeverity,
};
use serde_json::Value;

pub(in crate::storage::postgres::repository_sqlx) fn proposed_action_status_to_db(
    value: &ProposedActionStatus,
) -> &'static str {
    match value {
        ProposedActionStatus::Draft => "draft",
        ProposedActionStatus::Published => "published",
        ProposedActionStatus::Superseded => "superseded",
        ProposedActionStatus::Withdrawn => "withdrawn",
    }
}

pub(in crate::storage::postgres::repository_sqlx) fn proposed_action_kind_to_db(
    value: &ProposedActionKind,
) -> (&'static str, Option<&str>) {
    match value {
        ProposedActionKind::CreateKrProgress => ("create_kr_progress", None),
        ProposedActionKind::UpdateKrProgress => ("update_kr_progress", None),
        ProposedActionKind::DeleteKrProgressDryRun => ("delete_kr_progress_dry_run", None),
        ProposedActionKind::Custom(custom) => ("custom", Some(custom.as_str())),
    }
}

pub(in crate::storage::postgres::repository_sqlx) fn risk_severity_to_db(
    value: &RiskSeverity,
) -> &'static str {
    match value {
        RiskSeverity::Low => "low",
        RiskSeverity::Medium => "medium",
        RiskSeverity::High => "high",
        RiskSeverity::Critical => "critical",
    }
}

pub(in crate::storage::postgres::repository_sqlx) fn proposed_action_decision_to_db(
    value: &ProposedActionDecision,
) -> (&'static str, Option<&Value>) {
    match value {
        ProposedActionDecision::Confirm => ("confirm", None),
        ProposedActionDecision::EditThenConfirm { edited_payload } => {
            ("edit_then_confirm", Some(edited_payload))
        }
        ProposedActionDecision::Reject => ("reject", None),
    }
}

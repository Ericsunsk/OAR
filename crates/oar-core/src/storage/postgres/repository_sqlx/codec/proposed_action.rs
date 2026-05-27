use super::super::{PgRepositoryResult, PostgresRepositoryError};
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

#[allow(dead_code)]
pub(in crate::storage::postgres::repository_sqlx) fn proposed_action_status_from_db(
    value: &str,
) -> PgRepositoryResult<ProposedActionStatus> {
    match value {
        "draft" => Ok(ProposedActionStatus::Draft),
        "published" => Ok(ProposedActionStatus::Published),
        "superseded" => Ok(ProposedActionStatus::Superseded),
        "withdrawn" => Ok(ProposedActionStatus::Withdrawn),
        other => Err(PostgresRepositoryError::UnknownProposedActionStatus(
            other.to_string(),
        )),
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

#[allow(dead_code)]
pub(in crate::storage::postgres::repository_sqlx) fn proposed_action_kind_from_db(
    kind: &str,
    custom_kind: Option<&str>,
) -> PgRepositoryResult<ProposedActionKind> {
    match kind {
        "create_kr_progress" => Ok(ProposedActionKind::CreateKrProgress),
        "update_kr_progress" => Ok(ProposedActionKind::UpdateKrProgress),
        "delete_kr_progress_dry_run" => Ok(ProposedActionKind::DeleteKrProgressDryRun),
        "custom" => Ok(ProposedActionKind::Custom(
            custom_kind.unwrap_or_default().to_string(),
        )),
        other => Err(PostgresRepositoryError::UnknownProposedActionKind(
            other.to_string(),
        )),
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

#[allow(dead_code)]
pub(in crate::storage::postgres::repository_sqlx) fn risk_severity_from_db(
    value: &str,
) -> PgRepositoryResult<RiskSeverity> {
    match value {
        "low" => Ok(RiskSeverity::Low),
        "medium" => Ok(RiskSeverity::Medium),
        "high" => Ok(RiskSeverity::High),
        "critical" => Ok(RiskSeverity::Critical),
        other => Err(PostgresRepositoryError::UnknownRiskSeverity(
            other.to_string(),
        )),
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

#[allow(dead_code)]
pub(in crate::storage::postgres::repository_sqlx) fn proposed_action_decision_from_db(
    value: &str,
    edited_payload: Option<Value>,
) -> PgRepositoryResult<ProposedActionDecision> {
    match value {
        "confirm" => Ok(ProposedActionDecision::Confirm),
        "edit_then_confirm" => Ok(ProposedActionDecision::EditThenConfirm {
            edited_payload: edited_payload.unwrap_or(Value::Null),
        }),
        "reject" => Ok(ProposedActionDecision::Reject),
        other => Err(PostgresRepositoryError::UnknownProposedActionDecision(
            other.to_string(),
        )),
    }
}

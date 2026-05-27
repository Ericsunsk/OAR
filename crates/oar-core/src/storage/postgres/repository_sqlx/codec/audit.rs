use super::super::{PgRepositoryResult, PostgresRepositoryError};
use crate::action::audit_event::{AuditActorKind, AuditEventType};

pub(in crate::storage::postgres::repository_sqlx) fn audit_actor_kind_to_db(
    kind: &AuditActorKind,
) -> &'static str {
    match kind {
        AuditActorKind::User => "user",
        AuditActorKind::Bot => "bot",
        AuditActorKind::App => "app",
        AuditActorKind::System => "system",
        AuditActorKind::Service => "service",
    }
}

pub(in crate::storage::postgres::repository_sqlx) fn audit_actor_kind_from_db(
    value: &str,
) -> PgRepositoryResult<AuditActorKind> {
    match value {
        "user" => Ok(AuditActorKind::User),
        "bot" => Ok(AuditActorKind::Bot),
        "app" => Ok(AuditActorKind::App),
        "system" => Ok(AuditActorKind::System),
        "service" => Ok(AuditActorKind::Service),
        other => Err(PostgresRepositoryError::UnknownAuditActorKind(
            other.to_string(),
        )),
    }
}

pub(in crate::storage::postgres::repository_sqlx) fn audit_event_type_to_db(
    event_type: &AuditEventType,
) -> &'static str {
    match event_type {
        AuditEventType::ProposedActionDecisionRecorded => "proposed_action_decision_recorded",
        AuditEventType::ConfirmedActionRecorded => "confirmed_action_recorded",
        AuditEventType::DryRunExecuted => "dry_run_executed",
        AuditEventType::ExecutionDenied => "execution_denied",
        AuditEventType::ExecutionSucceeded => "execution_succeeded",
        AuditEventType::ExecutionFailed => "execution_failed",
    }
}

pub(in crate::storage::postgres::repository_sqlx) fn audit_event_type_from_db(
    value: &str,
) -> PgRepositoryResult<AuditEventType> {
    match value {
        "proposed_action_decision_recorded" => Ok(AuditEventType::ProposedActionDecisionRecorded),
        "confirmed_action_recorded" => Ok(AuditEventType::ConfirmedActionRecorded),
        "dry_run_executed" => Ok(AuditEventType::DryRunExecuted),
        "execution_denied" => Ok(AuditEventType::ExecutionDenied),
        "execution_succeeded" => Ok(AuditEventType::ExecutionSucceeded),
        "execution_failed" => Ok(AuditEventType::ExecutionFailed),
        other => Err(PostgresRepositoryError::UnknownAuditEventType(
            other.to_string(),
        )),
    }
}

use super::super::{PgRepositoryResult, PostgresRepositoryError};
use crate::domain::evidence::{EvidenceSourceKind, EvidenceVisibilityScope};

pub(in crate::storage::postgres::repository_sqlx) fn evidence_source_kind_to_db(
    value: &EvidenceSourceKind,
) -> &'static str {
    match value {
        EvidenceSourceKind::OkrProgress => "okr_progress",
        EvidenceSourceKind::LarkMinutes => "lark_minutes",
        EvidenceSourceKind::LarkDoc => "lark_doc",
        EvidenceSourceKind::LarkTask => "lark_task",
        EvidenceSourceKind::LarkCalendar => "lark_calendar",
        EvidenceSourceKind::LarkIm => "lark_im",
        EvidenceSourceKind::ManualReviewNote => "manual_review_note",
        EvidenceSourceKind::AuditEvent => "audit_event",
    }
}

pub(in crate::storage::postgres::repository_sqlx) fn evidence_source_kind_from_db(
    value: &str,
) -> PgRepositoryResult<EvidenceSourceKind> {
    match value {
        "okr_progress" => Ok(EvidenceSourceKind::OkrProgress),
        "lark_minutes" => Ok(EvidenceSourceKind::LarkMinutes),
        "lark_doc" => Ok(EvidenceSourceKind::LarkDoc),
        "lark_task" => Ok(EvidenceSourceKind::LarkTask),
        "lark_calendar" => Ok(EvidenceSourceKind::LarkCalendar),
        "lark_im" => Ok(EvidenceSourceKind::LarkIm),
        "manual_review_note" => Ok(EvidenceSourceKind::ManualReviewNote),
        "audit_event" => Ok(EvidenceSourceKind::AuditEvent),
        other => Err(PostgresRepositoryError::UnknownEvidenceSourceKind(
            other.to_string(),
        )),
    }
}

pub(in crate::storage::postgres::repository_sqlx) fn evidence_visibility_scope_to_db(
    value: &EvidenceVisibilityScope,
) -> &'static str {
    match value {
        EvidenceVisibilityScope::Tenant => "tenant",
        EvidenceVisibilityScope::Team => "team",
        EvidenceVisibilityScope::User => "user",
    }
}

pub(in crate::storage::postgres::repository_sqlx) fn evidence_visibility_scope_from_db(
    value: &str,
) -> PgRepositoryResult<EvidenceVisibilityScope> {
    match value {
        "tenant" => Ok(EvidenceVisibilityScope::Tenant),
        "team" => Ok(EvidenceVisibilityScope::Team),
        "user" => Ok(EvidenceVisibilityScope::User),
        other => Err(PostgresRepositoryError::UnknownEvidenceVisibilityScope(
            other.to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_source_kind_codec_round_trips_extended_lark_sources() {
        for (kind, db_value) in [
            (EvidenceSourceKind::LarkTask, "lark_task"),
            (EvidenceSourceKind::LarkCalendar, "lark_calendar"),
            (EvidenceSourceKind::LarkIm, "lark_im"),
        ] {
            assert_eq!(evidence_source_kind_to_db(&kind), db_value);
            assert_eq!(evidence_source_kind_from_db(db_value).expect("kind"), kind);
        }
    }
}

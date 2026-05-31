use oar_core::domain::evidence::{EvidenceSourceKind, EvidenceVisibilityScope};
use oar_core::domain::proposed_action::{ProposedActionKind, ProposedActionStatus, RiskSeverity};
use oar_core::storage::postgres::{
    StoredProposedActionDecisionKind, StoredReviewInboxLedgerStage, StoredReviewInboxLedgerStatus,
};

pub(super) fn proposed_action_status(status: ProposedActionStatus) -> &'static str {
    match status {
        ProposedActionStatus::Draft => "draft",
        ProposedActionStatus::Published => "published",
        ProposedActionStatus::Superseded => "superseded",
        ProposedActionStatus::Withdrawn => "withdrawn",
    }
}

pub(super) fn proposed_action_kind(kind: &ProposedActionKind) -> String {
    match kind {
        ProposedActionKind::CreateKrProgress => "create_kr_progress".to_string(),
        ProposedActionKind::UpdateKrProgress => "update_kr_progress".to_string(),
        ProposedActionKind::DeleteKrProgressDryRun => "delete_kr_progress_dry_run".to_string(),
        ProposedActionKind::Custom(custom)
            if matches!(
                custom.as_str(),
                "ping_owner" | "create_task" | "schedule_review"
            ) =>
        {
            custom.clone()
        }
        ProposedActionKind::Custom(_) => "custom".to_string(),
    }
}

pub(super) fn risk_severity(severity: RiskSeverity) -> &'static str {
    match severity {
        RiskSeverity::Low => "low",
        RiskSeverity::Medium => "medium",
        RiskSeverity::High => "high",
        RiskSeverity::Critical => "critical",
    }
}

pub(super) fn proposed_action_decision(decision: StoredProposedActionDecisionKind) -> &'static str {
    match decision {
        StoredProposedActionDecisionKind::Confirm => "confirm",
        StoredProposedActionDecisionKind::EditThenConfirm => "edit_then_confirm",
        StoredProposedActionDecisionKind::Reject => "reject",
    }
}

pub(super) fn ledger_stage(stage: StoredReviewInboxLedgerStage) -> &'static str {
    match stage {
        StoredReviewInboxLedgerStage::ConfirmedAction => "confirmed_action",
        StoredReviewInboxLedgerStage::OperationLedger => "operation_ledger",
        StoredReviewInboxLedgerStage::PlatformAdapter => "platform_adapter",
        StoredReviewInboxLedgerStage::AuditEvent => "audit_event",
    }
}

pub(super) fn ledger_status(status: StoredReviewInboxLedgerStatus) -> &'static str {
    match status {
        StoredReviewInboxLedgerStatus::Pending => "pending",
        StoredReviewInboxLedgerStatus::Ok => "ok",
        StoredReviewInboxLedgerStatus::Error => "error",
    }
}

pub(super) fn evidence_source_kind(source: EvidenceSourceKind) -> &'static str {
    match source {
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

pub(super) fn evidence_visibility(visibility: EvidenceVisibilityScope) -> &'static str {
    match visibility {
        EvidenceVisibilityScope::Tenant => "tenant",
        EvidenceVisibilityScope::Team => "team",
        EvidenceVisibilityScope::User => "user",
    }
}

pub(super) fn signal_type(value: &str) -> &'static str {
    match value {
        "progress" => "progress",
        "blocker" => "blocker",
        "dependency" => "dependency",
        _ => "cadence",
    }
}

pub(super) fn default_signal_type(source: EvidenceSourceKind) -> &'static str {
    match source {
        EvidenceSourceKind::OkrProgress => "progress",
        EvidenceSourceKind::LarkTask => "blocker",
        _ => "cadence",
    }
}

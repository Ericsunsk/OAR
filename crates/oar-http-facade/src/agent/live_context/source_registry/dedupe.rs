use super::super::refs::{
    parse_calendar_evidence_ref, parse_doc_evidence_ref, parse_minutes_evidence_ref,
    parse_okr_evidence_ref, parse_task_evidence_ref,
};
use super::source_types::{
    is_calendar_source_type, is_doc_source_type, is_minutes_source_type, is_okr_source_type,
    is_task_source_type,
};
use crate::agent::request::AgentEvidenceRefDTO;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum EvidenceRefKey {
    Okr {
        okr_id: String,
        objective_id: String,
        kr_id: String,
    },
    Task {
        source_ref: String,
    },
    Calendar {
        source_ref: String,
    },
    Doc {
        source_ref: String,
    },
    Minutes {
        source_ref: String,
    },
    Raw {
        source_type: String,
        source_ref: String,
    },
}

pub(super) fn evidence_ref_key(evidence_ref: &AgentEvidenceRefDTO) -> EvidenceRefKey {
    let source_type = evidence_ref.source_type.trim().to_ascii_lowercase();
    if is_okr_source_type(&source_type) {
        if let Some(parsed) = parse_okr_evidence_ref(&evidence_ref.source_ref) {
            return EvidenceRefKey::Okr {
                okr_id: parsed.okr_id,
                objective_id: parsed.objective_id,
                kr_id: parsed.kr_id,
            };
        }
    }
    if is_task_source_type(&source_type) {
        if let Some(parsed) = parse_task_evidence_ref(&evidence_ref.source_ref) {
            return EvidenceRefKey::Task {
                source_ref: parsed.source_ref,
            };
        }
    }
    if is_calendar_source_type(&source_type) {
        if let Some(parsed) = parse_calendar_evidence_ref(&evidence_ref.source_ref) {
            return EvidenceRefKey::Calendar {
                source_ref: parsed.source_ref(),
            };
        }
    }
    if is_doc_source_type(&source_type) {
        if let Some(parsed) = parse_doc_evidence_ref(&evidence_ref.source_ref) {
            return EvidenceRefKey::Doc {
                source_ref: parsed.source_ref(),
            };
        }
    }
    if is_minutes_source_type(&source_type) {
        if let Some(parsed) = parse_minutes_evidence_ref(&evidence_ref.source_ref) {
            return EvidenceRefKey::Minutes {
                source_ref: parsed.source_ref(),
            };
        }
    }
    EvidenceRefKey::Raw {
        source_type,
        source_ref: evidence_ref.source_ref.trim().to_string(),
    }
}

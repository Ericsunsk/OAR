use super::refs::{
    parse_calendar_evidence_ref, parse_doc_evidence_ref, parse_minutes_evidence_ref,
    parse_okr_evidence_ref, parse_task_evidence_ref, ParsedCalendarEvidenceRef,
    ParsedDocEvidenceRef, ParsedMinutesEvidenceRef, ParsedOkrEvidenceRef, ParsedTaskEvidenceRef,
};
use super::summary::degraded_summary;
use crate::agent::request::AgentEvidenceRefDTO;
use std::collections::HashSet;

mod dedupe;
mod source_types;

use dedupe::evidence_ref_key;
use source_types::{
    is_calendar_source_type, is_doc_source_type, is_minutes_source_type, is_okr_source_type,
    is_task_source_type,
};

#[derive(Debug, Default)]
pub(super) struct LiveEvidenceResolution<'a> {
    pub(super) okr_refs: Vec<(&'a AgentEvidenceRefDTO, ParsedOkrEvidenceRef)>,
    pub(super) task_refs: Vec<(&'a AgentEvidenceRefDTO, ParsedTaskEvidenceRef)>,
    pub(super) calendar_refs: Vec<(&'a AgentEvidenceRefDTO, ParsedCalendarEvidenceRef)>,
    pub(super) doc_refs: Vec<(&'a AgentEvidenceRefDTO, ParsedDocEvidenceRef)>,
    pub(super) minutes_refs: Vec<(&'a AgentEvidenceRefDTO, ParsedMinutesEvidenceRef)>,
    pub(super) degraded: Vec<String>,
}

pub(super) fn resolve_evidence_refs<'a>(
    evidence_refs: &'a [AgentEvidenceRefDTO],
    limit: usize,
) -> LiveEvidenceResolution<'a> {
    let mut resolution = LiveEvidenceResolution::default();
    let mut seen_refs = HashSet::new();
    let mut duplicate_count = 0usize;
    let mut processed_count = 0usize;
    let mut skipped_over_limit = false;

    for evidence_ref in evidence_refs {
        if !seen_refs.insert(evidence_ref_key(evidence_ref)) {
            duplicate_count += 1;
            continue;
        }
        if processed_count >= limit {
            skipped_over_limit = true;
            continue;
        }
        processed_count += 1;

        if is_okr_source_type(&evidence_ref.source_type) {
            match parse_okr_evidence_ref(&evidence_ref.source_ref) {
                Some(parsed) => resolution.okr_refs.push((evidence_ref, parsed)),
                None => resolution.degraded.push(degraded_summary(
                    evidence_ref,
                    "source_ref 不是可识别的 OKR 引用",
                )),
            }
            continue;
        }

        if is_task_source_type(&evidence_ref.source_type) {
            match parse_task_evidence_ref(&evidence_ref.source_ref) {
                Some(parsed) => resolution.task_refs.push((evidence_ref, parsed)),
                None => resolution.degraded.push(degraded_summary(
                    evidence_ref,
                    "source_ref 不是可识别的任务引用",
                )),
            }
            continue;
        }

        if is_calendar_source_type(&evidence_ref.source_type) {
            match parse_calendar_evidence_ref(&evidence_ref.source_ref) {
                Some(parsed) => resolution.calendar_refs.push((evidence_ref, parsed)),
                None => resolution.degraded.push(degraded_summary(
                    evidence_ref,
                    "source_ref 不是可识别的日历引用",
                )),
            }
            continue;
        }

        if is_doc_source_type(&evidence_ref.source_type) {
            match parse_doc_evidence_ref(&evidence_ref.source_ref) {
                Some(parsed) => resolution.doc_refs.push((evidence_ref, parsed)),
                None => resolution.degraded.push(degraded_summary(
                    evidence_ref,
                    "source_ref 不是可识别的文档引用",
                )),
            }
            continue;
        }

        if is_minutes_source_type(&evidence_ref.source_type) {
            match parse_minutes_evidence_ref(&evidence_ref.source_ref) {
                Some(parsed) => resolution.minutes_refs.push((evidence_ref, parsed)),
                None => resolution.degraded.push(degraded_summary(
                    evidence_ref,
                    "source_ref 不是可识别的妙记引用",
                )),
            }
            continue;
        }

        resolution.degraded.push(degraded_summary(
            evidence_ref,
            "source_type 暂不支持实时读取",
        ));
    }

    if duplicate_count > 0 {
        resolution
            .degraded
            .push(format!("已合并 {} 条重复 evidence refs。", duplicate_count));
    }

    if skipped_over_limit {
        resolution
            .degraded
            .push(format!("仅实时读取前 {} 条 evidence refs。", limit));
    }

    resolution
}

#[cfg(test)]
mod tests;

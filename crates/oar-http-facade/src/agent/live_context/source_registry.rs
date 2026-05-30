use super::refs::{
    parse_okr_evidence_ref, parse_task_evidence_ref, ParsedOkrEvidenceRef, ParsedTaskEvidenceRef,
};
use super::summary::degraded_summary;
use crate::agent::request::AgentEvidenceRefDTO;

#[derive(Debug, Default)]
pub(super) struct LiveEvidenceResolution<'a> {
    pub(super) okr_refs: Vec<(&'a AgentEvidenceRefDTO, ParsedOkrEvidenceRef)>,
    pub(super) task_refs: Vec<(&'a AgentEvidenceRefDTO, ParsedTaskEvidenceRef)>,
    pub(super) degraded: Vec<String>,
}

pub(super) fn resolve_evidence_refs<'a>(
    evidence_refs: &'a [AgentEvidenceRefDTO],
    limit: usize,
) -> LiveEvidenceResolution<'a> {
    let mut resolution = LiveEvidenceResolution::default();

    for evidence_ref in evidence_refs.iter().take(limit) {
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

        resolution.degraded.push(degraded_summary(
            evidence_ref,
            "source_type 暂不支持实时读取",
        ));
    }

    if evidence_refs.len() > limit {
        resolution
            .degraded
            .push(format!("仅实时读取前 {} 条 evidence refs。", limit));
    }

    resolution
}

fn is_okr_source_type(source_type: &str) -> bool {
    let source_type = source_type.trim().to_ascii_lowercase();
    source_type == "okr" || source_type == "feishu_okr" || source_type == "lark_okr"
}

fn is_task_source_type(source_type: &str) -> bool {
    let source_type = source_type.trim().to_ascii_lowercase();
    source_type == "task" || source_type == "feishu_task" || source_type == "lark_task"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_mixed_evidence_refs_without_cross_parsing_sources() {
        let refs = vec![
            evidence_ref(
                "okr",
                "okr://okr_demo/objectives/obj_demo/krs/kr_demo",
                "OKR evidence",
            ),
            evidence_ref("task", "task://task_123", "Task evidence"),
            evidence_ref("doc", "task://task_456", "Doc evidence"),
            evidence_ref("okr", "task://task_789", "Invalid OKR evidence"),
            evidence_ref("task", "task://task_over_limit", "Too late"),
        ];

        let resolution = resolve_evidence_refs(&refs, 4);

        assert_eq!(resolution.okr_refs.len(), 1);
        assert_eq!(resolution.okr_refs[0].1.okr_id, "okr_demo");
        assert_eq!(resolution.task_refs.len(), 1);
        assert_eq!(resolution.task_refs[0].1.task_id, "task_123");
        assert_eq!(resolution.degraded.len(), 3);
        assert!(resolution
            .degraded
            .iter()
            .any(|summary| summary.contains("source_type 暂不支持实时读取")));
        assert!(resolution
            .degraded
            .iter()
            .any(|summary| summary.contains("source_ref 不是可识别的 OKR 引用")));
        assert!(resolution
            .degraded
            .iter()
            .any(|summary| summary.contains("仅实时读取前 4 条 evidence refs")));
        assert!(!resolution
            .degraded
            .iter()
            .any(|summary| summary.contains("Too late")));
    }

    #[test]
    fn resolves_encoded_okr_refs_to_raw_ids() {
        let refs = vec![evidence_ref(
            "okr",
            "okr://okr%3A1/objectives/obj%2F1/krs/kr%20a%25%3F%23%3A",
            "Encoded OKR evidence",
        )];

        let resolution = resolve_evidence_refs(&refs, 4);

        assert_eq!(resolution.okr_refs.len(), 1);
        assert_eq!(resolution.okr_refs[0].1.okr_id, "okr:1");
        assert_eq!(resolution.okr_refs[0].1.objective_id, "obj/1");
        assert_eq!(resolution.okr_refs[0].1.kr_id, "kr a%?#:");
        assert!(resolution.degraded.is_empty());
    }

    #[test]
    fn invalid_refs_degrade_without_echoing_evidence_summary_or_ref() {
        let refs = vec![evidence_ref(
            "task",
            "task://sk-secret-ref/subtask",
            "sk-secret auth code raw transcript",
        )];

        let resolution = resolve_evidence_refs(&refs, 4);

        assert_eq!(resolution.degraded.len(), 1);
        assert!(resolution.degraded[0].contains("source_ref 不是可识别的任务引用"));
        assert!(!resolution.degraded[0].contains("sk-secret"));
        assert!(!resolution.degraded[0].contains("auth code"));
        assert!(!resolution.degraded[0].contains("raw transcript"));
    }

    #[test]
    fn invalid_okr_refs_degrade_without_echoing_evidence_summary_or_ref() {
        let refs = vec![evidence_ref(
            "okr",
            "okr://sk-secret-ref/objectives/obj_demo/krs/kr%",
            "sk-secret auth code raw transcript",
        )];

        let resolution = resolve_evidence_refs(&refs, 4);

        assert_eq!(resolution.degraded.len(), 1);
        assert!(resolution.degraded[0].contains("source_ref 不是可识别的 OKR 引用"));
        assert!(!resolution.degraded[0].contains("sk-secret"));
        assert!(!resolution.degraded[0].contains("auth code"));
        assert!(!resolution.degraded[0].contains("raw transcript"));
    }

    fn evidence_ref(source_type: &str, source_ref: &str, summary: &str) -> AgentEvidenceRefDTO {
        AgentEvidenceRefDTO {
            source_type: source_type.to_string(),
            source_ref: source_ref.to_string(),
            summary: summary.to_string(),
        }
    }
}

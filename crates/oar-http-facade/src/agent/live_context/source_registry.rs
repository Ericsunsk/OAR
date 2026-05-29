use oar_core::action::capability::FeishuScope;

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

pub(super) fn gate_evidence_refs_by_scope(
    scopes: &[String],
    resolution: &mut LiveEvidenceResolution<'_>,
) {
    if !resolution.okr_refs.is_empty() && !has_okr_evidence_read_scopes(scopes) {
        resolution.degraded.push(
            "未读取到实时 Feishu OKR 证据：授权缺少 OKR 内容或 progress 读取权限。".to_string(),
        );
        resolution.okr_refs.clear();
    }
    if !resolution.task_refs.is_empty() && !has_task_read_scope(scopes) {
        resolution
            .degraded
            .push("未读取到实时 Feishu 任务证据：授权缺少任务读取权限。".to_string());
        resolution.task_refs.clear();
    }
}

fn has_okr_evidence_read_scopes(scopes: &[String]) -> bool {
    has_feishu_scope(scopes, FeishuScope::OkrContentRead)
        && has_feishu_scope(scopes, FeishuScope::OkrProgressRead)
}

fn has_task_read_scope(scopes: &[String]) -> bool {
    has_feishu_scope(scopes, FeishuScope::TaskRead)
}

fn has_feishu_scope(scopes: &[String], required: FeishuScope) -> bool {
    let required = required.as_str();
    scopes.iter().any(|scope| scope.trim() == required)
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
    use oar_core::action::capability::OarRequiredScope;

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
    fn scope_gate_clears_only_sources_missing_their_real_feishu_scope() {
        let refs = vec![
            evidence_ref(
                "okr",
                "okr://okr_demo/objectives/obj_demo/krs/kr_demo",
                "OKR evidence",
            ),
            evidence_ref("task", "task://task_123", "Task evidence"),
        ];
        let mut resolution = resolve_evidence_refs(&refs, 4);

        gate_evidence_refs_by_scope(
            &[
                FeishuScope::OkrContentRead.as_str().to_string(),
                FeishuScope::OkrProgressRead.as_str().to_string(),
            ],
            &mut resolution,
        );

        assert_eq!(resolution.okr_refs.len(), 1);
        assert!(resolution.task_refs.is_empty());
        assert!(resolution
            .degraded
            .iter()
            .any(|summary| summary.contains("授权缺少任务读取权限")));

        let mut resolution = resolve_evidence_refs(&refs, 4);
        gate_evidence_refs_by_scope(
            &[
                FeishuScope::OkrContentRead.as_str().to_string(),
                FeishuScope::TaskRead.as_str().to_string(),
            ],
            &mut resolution,
        );

        assert!(resolution.okr_refs.is_empty());
        assert_eq!(resolution.task_refs.len(), 1);
        assert!(resolution
            .degraded
            .iter()
            .any(|summary| summary.contains("授权缺少 OKR 内容或 progress 读取权限")));
        assert!(!resolution
            .degraded
            .iter()
            .any(|summary| summary.contains("OKR evidence") || summary.contains("okr_demo")));
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
    fn okr_evidence_read_scope_requires_content_and_progress_feishu_scope_names() {
        assert!(has_okr_evidence_read_scopes(&[
            FeishuScope::OkrContentRead.as_str().to_string(),
            FeishuScope::OkrProgressRead.as_str().to_string(),
        ]));
        assert!(!has_okr_evidence_read_scopes(&[
            FeishuScope::OkrContentRead.as_str().to_string()
        ]));
        assert!(!has_okr_evidence_read_scopes(&[
            FeishuScope::OkrProgressRead.as_str().to_string()
        ]));
        assert!(!has_okr_evidence_read_scopes(&[
            OarRequiredScope::OkrContentRead.as_str().to_string(),
            OarRequiredScope::OkrProgressRead.as_str().to_string(),
        ]));
        assert!(!has_okr_evidence_read_scopes(&[
            "task:task:read".to_string()
        ]));
    }

    #[test]
    fn task_read_scope_accepts_only_feishu_scope_name() {
        assert!(has_task_read_scope(&[FeishuScope::TaskRead
            .as_str()
            .to_string()]));
        assert!(!has_task_read_scope(&["task.read".to_string()]));
        assert!(!has_task_read_scope(&[FeishuScope::OkrProgressRead
            .as_str()
            .to_string()]));
    }

    fn evidence_ref(source_type: &str, source_ref: &str, summary: &str) -> AgentEvidenceRefDTO {
        AgentEvidenceRefDTO {
            source_type: source_type.to_string(),
            source_ref: source_ref.to_string(),
            summary: summary.to_string(),
        }
    }
}

use std::collections::HashSet;

use oar_core::action::capability::FeishuScope;
use oar_lark_adapter::DocSourceRefKind;

use super::source_registry::LiveEvidenceResolution;
use super::status::LiveFeishuReadStatus;
use crate::agent::tools::AgentReadTool;

const MINUTES_READONLY_SCOPE_COMPAT: &str = "minutes:minutes:readonly";

pub(super) fn gate_read_demand_by_scope(
    scopes: &[String],
    evidence_resolution: &mut LiveEvidenceResolution<'_>,
    read_tools: &mut Vec<AgentReadTool>,
    degraded_read_statuses: &mut Vec<LiveFeishuReadStatus>,
) -> bool {
    gate_evidence_refs_by_scope(scopes, evidence_resolution);
    gate_read_tools_by_scope(scopes, read_tools, degraded_read_statuses);
    !(evidence_resolution.okr_refs.is_empty()
        && evidence_resolution.task_refs.is_empty()
        && evidence_resolution.calendar_refs.is_empty()
        && evidence_resolution.doc_refs.is_empty()
        && evidence_resolution.minutes_refs.is_empty()
        && read_tools.is_empty())
}

pub(super) fn gate_read_tools_by_scope(
    scopes: &[String],
    read_tools: &mut Vec<AgentReadTool>,
    degraded: &mut Vec<LiveFeishuReadStatus>,
) {
    dedupe_read_tools(read_tools);
    read_tools.retain(|tool| {
        let spec = tool.spec();
        let required_scopes = match spec.required_feishu_scope_names() {
            Ok(scopes) => scopes,
            Err(error) => {
                let reason = error.safe_reason();
                degraded.push(LiveFeishuReadStatus::degraded_for_tool(*tool, &reason));
                return false;
            }
        };
        let missing = missing_feishu_scope_names(scopes, &required_scopes);
        if missing.is_empty() {
            return true;
        }
        degraded.push(LiveFeishuReadStatus::degraded_for_tool(
            *tool,
            &format!("授权缺少 {}", missing.join("、")),
        ));
        false
    });
}

fn dedupe_read_tools(read_tools: &mut Vec<AgentReadTool>) {
    let mut seen = HashSet::new();
    read_tools.retain(|tool| seen.insert(*tool));
}

fn gate_evidence_refs_by_scope(scopes: &[String], resolution: &mut LiveEvidenceResolution<'_>) {
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
    if !resolution.calendar_refs.is_empty() && !has_calendar_evidence_read_scopes(scopes) {
        resolution
            .degraded
            .push("未读取到实时 Feishu 日历证据：授权缺少日历或日程读取权限。".to_string());
        resolution.calendar_refs.clear();
    }
    if !resolution.doc_refs.is_empty() {
        gate_doc_evidence_refs_by_scope(scopes, resolution);
    }
    if !resolution.minutes_refs.is_empty() && !has_minutes_basic_read_scope(scopes) {
        resolution
            .degraded
            .push("未读取到实时 Feishu 妙记证据：授权缺少妙记基础信息读取权限。".to_string());
        resolution.minutes_refs.clear();
    }
}

fn gate_doc_evidence_refs_by_scope(scopes: &[String], resolution: &mut LiveEvidenceResolution<'_>) {
    if !has_feishu_scope(scopes, FeishuScope::DocxDocumentRead) {
        resolution
            .degraded
            .push("未读取到实时 Feishu 文档证据：授权缺少新版文档读取权限。".to_string());
        resolution.doc_refs.clear();
        return;
    }

    if has_feishu_scope(scopes, FeishuScope::WikiNodeRead) {
        return;
    }

    let before = resolution.doc_refs.len();
    resolution
        .doc_refs
        .retain(|(_, parsed)| !matches!(parsed.kind, DocSourceRefKind::Wiki { .. }));
    if resolution.doc_refs.len() != before {
        resolution
            .degraded
            .push("未读取到实时 Feishu 知识库文档证据：授权缺少知识库节点读取权限。".to_string());
    }
}

fn has_okr_evidence_read_scopes(scopes: &[String]) -> bool {
    has_feishu_scope(scopes, FeishuScope::OkrContentRead)
        && has_feishu_scope(scopes, FeishuScope::OkrProgressRead)
}

fn has_task_read_scope(scopes: &[String]) -> bool {
    has_feishu_scope(scopes, FeishuScope::TaskRead)
}

fn has_calendar_evidence_read_scopes(scopes: &[String]) -> bool {
    has_feishu_scope(scopes, FeishuScope::CalendarRead)
        && has_feishu_scope(scopes, FeishuScope::CalendarEventRead)
}

fn has_minutes_basic_read_scope(scopes: &[String]) -> bool {
    has_feishu_scope(scopes, FeishuScope::MinutesBasicRead)
        || scopes
            .iter()
            .any(|scope| scope.trim() == MINUTES_READONLY_SCOPE_COMPAT)
}

fn missing_feishu_scope_names<'a>(
    scopes: &[String],
    required_scopes: &'a [&'static str],
) -> Vec<&'a str> {
    required_scopes
        .iter()
        .filter_map(|required| {
            if scopes.iter().any(|scope| scope.trim() == *required) {
                None
            } else {
                Some(*required)
            }
        })
        .collect()
}

fn has_feishu_scope(scopes: &[String], required: FeishuScope) -> bool {
    let required = required.as_str();
    scopes.iter().any(|scope| scope.trim() == required)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::live_context::source_registry::resolve_evidence_refs;
    use crate::agent::request::AgentEvidenceRefDTO;
    use oar_core::action::capability::OarRequiredScope;

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
    fn scope_gate_requires_calendar_and_event_scopes_for_calendar_evidence_refs() {
        let refs = vec![evidence_ref(
            "calendar",
            "calendar://cal_secret/events/evt_secret",
            "Calendar evidence sk-secret",
        )];

        let mut resolution = resolve_evidence_refs(&refs, 4);
        gate_evidence_refs_by_scope(
            &[FeishuScope::CalendarRead.as_str().to_string()],
            &mut resolution,
        );

        assert!(resolution.calendar_refs.is_empty());
        assert!(resolution
            .degraded
            .iter()
            .any(|summary| summary.contains("授权缺少日历或日程读取权限")));
        assert!(!resolution
            .degraded
            .iter()
            .any(|summary| summary.contains("cal_secret") || summary.contains("sk-secret")));

        let mut resolution = resolve_evidence_refs(&refs, 4);
        gate_evidence_refs_by_scope(
            &[
                FeishuScope::CalendarRead.as_str().to_string(),
                FeishuScope::CalendarEventRead.as_str().to_string(),
            ],
            &mut resolution,
        );

        assert_eq!(resolution.calendar_refs.len(), 1);
        assert!(resolution.degraded.is_empty());
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

    #[test]
    fn calendar_evidence_read_scope_requires_calendar_and_event_feishu_scope_names() {
        assert!(has_calendar_evidence_read_scopes(&[
            FeishuScope::CalendarRead.as_str().to_string(),
            FeishuScope::CalendarEventRead.as_str().to_string(),
        ]));
        assert!(!has_calendar_evidence_read_scopes(&[
            FeishuScope::CalendarRead.as_str().to_string()
        ]));
        assert!(!has_calendar_evidence_read_scopes(&[
            FeishuScope::CalendarEventRead.as_str().to_string()
        ]));
        assert!(!has_calendar_evidence_read_scopes(&[
            FeishuScope::CalendarFreeBusyRead.as_str().to_string()
        ]));
    }

    #[test]
    fn doc_evidence_scope_requires_docx_and_only_requires_wiki_for_wiki_refs() {
        let refs = vec![
            evidence_ref("doc", "docx://doxcni6mOy7jLRWbEylaKKabcef", "Doc evidence"),
            evidence_ref("wiki", "wiki://wikcnKQ1k3p8Vabcef", "Wiki evidence"),
        ];
        let mut resolution = resolve_evidence_refs(&refs, 4);

        gate_evidence_refs_by_scope(
            &[FeishuScope::DocxDocumentRead.as_str().to_string()],
            &mut resolution,
        );

        assert_eq!(resolution.doc_refs.len(), 1);
        assert_eq!(
            resolution.doc_refs[0].1.source_ref(),
            "docx://doxcni6mOy7jLRWbEylaKKabcef"
        );
        assert!(resolution
            .degraded
            .iter()
            .any(|summary| summary.contains("授权缺少知识库节点读取权限")));

        let mut resolution = resolve_evidence_refs(&refs, 4);
        gate_evidence_refs_by_scope(
            &[
                FeishuScope::DocxDocumentRead.as_str().to_string(),
                FeishuScope::WikiNodeRead.as_str().to_string(),
            ],
            &mut resolution,
        );

        assert_eq!(resolution.doc_refs.len(), 2);
        assert!(resolution.degraded.is_empty());
    }

    #[test]
    fn minutes_evidence_scope_accepts_basic_or_readonly_scope_names() {
        let refs = vec![evidence_ref(
            "meeting",
            "minutes://obcnq3b9jl72l83w4f14xxxx",
            "Minutes evidence",
        )];

        let mut resolution = resolve_evidence_refs(&refs, 4);
        gate_evidence_refs_by_scope(&[], &mut resolution);

        assert!(resolution.minutes_refs.is_empty());
        assert!(resolution
            .degraded
            .iter()
            .any(|summary| summary.contains("授权缺少妙记基础信息读取权限")));

        let mut resolution = resolve_evidence_refs(&refs, 4);
        gate_evidence_refs_by_scope(
            &[FeishuScope::MinutesBasicRead.as_str().to_string()],
            &mut resolution,
        );

        assert_eq!(resolution.minutes_refs.len(), 1);
        assert!(resolution.degraded.is_empty());

        let mut resolution = resolve_evidence_refs(&refs, 4);
        gate_evidence_refs_by_scope(
            &[MINUTES_READONLY_SCOPE_COMPAT.to_string()],
            &mut resolution,
        );

        assert_eq!(resolution.minutes_refs.len(), 1);
        assert!(resolution.degraded.is_empty());
        assert!(!has_minutes_basic_read_scope(&[
            "minutes.basic.read".to_string()
        ]));
    }

    fn evidence_ref(source_type: &str, source_ref: &str, summary: &str) -> AgentEvidenceRefDTO {
        AgentEvidenceRefDTO {
            source_type: source_type.to_string(),
            source_ref: source_ref.to_string(),
            summary: summary.to_string(),
        }
    }
}

use oar_core::action::capability::FeishuScope;
use oar_lark_adapter::parse_task_source_ref;

use crate::agent::request::AgentEvidenceRefDTO;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParsedOkrEvidenceRef {
    pub(super) okr_id: String,
    pub(super) objective_id: String,
    pub(super) kr_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParsedTaskEvidenceRef {
    pub(super) source_ref: String,
    pub(super) task_id: String,
}

pub(super) fn parse_okr_evidence_ref(source_ref: &str) -> Option<ParsedOkrEvidenceRef> {
    let trimmed = source_ref.trim();
    if let Some(path_like) = trimmed.strip_prefix("okr://") {
        return parse_path_style_ref(path_like);
    }
    if let Some(value) = trimmed.strip_prefix("okr:") {
        return parse_colon_style_ref(value);
    }
    None
}

pub(super) fn parse_task_evidence_ref(source_ref: &str) -> Option<ParsedTaskEvidenceRef> {
    let trimmed = source_ref.trim();
    let normalized = if trimmed.starts_with("task://") {
        trimmed.to_string()
    } else if let Some(task_id) = trimmed.strip_prefix("feishu://task/") {
        format!("task://{}", task_id.trim())
    } else {
        return None;
    };

    let parsed = parse_task_source_ref(&normalized).ok()?;
    Some(ParsedTaskEvidenceRef {
        source_ref: normalized,
        task_id: parsed.task_id,
    })
}

pub(super) fn is_okr_source_type(source_type: &str) -> bool {
    let source_type = source_type.trim().to_ascii_lowercase();
    source_type == "okr" || source_type == "feishu_okr" || source_type == "lark_okr"
}

pub(super) fn is_task_source_type(source_type: &str) -> bool {
    let source_type = source_type.trim().to_ascii_lowercase();
    source_type == "task" || source_type == "feishu_task" || source_type == "lark_task"
}

pub(super) fn gate_refs_by_scope<'a>(
    scopes: &[String],
    okr_refs: &mut Vec<(&'a AgentEvidenceRefDTO, ParsedOkrEvidenceRef)>,
    task_refs: &mut Vec<(&'a AgentEvidenceRefDTO, ParsedTaskEvidenceRef)>,
    degraded: &mut Vec<String>,
) {
    if !okr_refs.is_empty() && !has_okr_progress_read_scope(scopes) {
        degraded.push("未读取到实时 Feishu OKR 证据：授权缺少 OKR 进展读取权限。".to_string());
        okr_refs.clear();
    }
    if !task_refs.is_empty() && !has_task_read_scope(scopes) {
        degraded.push("未读取到实时 Feishu 任务证据：授权缺少任务读取权限。".to_string());
        task_refs.clear();
    }
}

pub(super) fn has_okr_progress_read_scope(scopes: &[String]) -> bool {
    let required = FeishuScope::OkrProgressRead.as_str();
    scopes.iter().any(|scope| scope.trim() == required)
}

pub(super) fn has_task_read_scope(scopes: &[String]) -> bool {
    let required = FeishuScope::TaskRead.as_str();
    scopes.iter().any(|scope| scope.trim() == required)
}

fn parse_path_style_ref(value: &str) -> Option<ParsedOkrEvidenceRef> {
    let segments = value
        .split('/')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.len() != 5 {
        return None;
    }
    if segments[1] != "objectives" || segments[3] != "krs" {
        return None;
    }
    if !valid_platform_ref_segment(segments[0])
        || !valid_platform_ref_segment(segments[2])
        || !valid_platform_ref_segment(segments[4])
    {
        return None;
    }
    Some(ParsedOkrEvidenceRef {
        okr_id: segments[0].to_string(),
        objective_id: segments[2].to_string(),
        kr_id: segments[4].to_string(),
    })
}

fn parse_colon_style_ref(value: &str) -> Option<ParsedOkrEvidenceRef> {
    let segments = value.split(':').map(str::trim).collect::<Vec<_>>();
    if segments.len() != 5 {
        return None;
    }
    if segments[1] != "objective" || segments[3] != "kr" {
        return None;
    }
    if !valid_platform_ref_segment(segments[0])
        || !valid_platform_ref_segment(segments[2])
        || !valid_platform_ref_segment(segments[4])
    {
        return None;
    }
    Some(ParsedOkrEvidenceRef {
        okr_id: segments[0].to_string(),
        objective_id: segments[2].to_string(),
        kr_id: segments[4].to_string(),
    })
}

fn valid_platform_ref_segment(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && trimmed.chars().count() <= 100
        && !trimmed.contains('/')
        && !trimmed.contains('?')
        && !trimmed.contains('#')
}

use oar_core::action::capability::FeishuScope;
use oar_lark_adapter::DocSourceRefKind;

use super::super::source_registry::LiveEvidenceResolution;
use super::scopes::{
    has_calendar_evidence_read_scopes, has_feishu_scope, has_minutes_basic_read_scope,
    has_okr_evidence_read_scopes, has_task_read_scope,
};

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

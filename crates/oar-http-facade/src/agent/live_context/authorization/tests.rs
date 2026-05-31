use crate::agent::live_context::source_registry::resolve_evidence_refs;
use crate::agent::request::AgentEvidenceRefDTO;
use oar_core::action::capability::{FeishuScope, OarRequiredScope};

use super::evidence::gate_evidence_refs_by_scope;
use super::scopes::{
    has_calendar_evidence_read_scopes, has_minutes_basic_read_scope, has_okr_evidence_read_scopes,
    has_task_read_scope, MINUTES_READONLY_SCOPE_COMPAT,
};

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

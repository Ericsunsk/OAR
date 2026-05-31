use super::*;
use crate::agent::request::AgentEvidenceRefDTO;

#[test]
fn resolves_mixed_evidence_refs_without_cross_parsing_sources() {
    let refs = vec![
        evidence_ref(
            "okr",
            "okr://okr_demo/objectives/obj_demo/krs/kr_demo",
            "OKR evidence",
        ),
        evidence_ref("task", "task://task_123", "Task evidence"),
        evidence_ref(
            "lark_calendar",
            "calendar://cal_1/events/evt_1",
            "Calendar evidence",
        ),
        evidence_ref("doc", "task://task_456", "Doc evidence"),
        evidence_ref("okr", "task://task_789", "Invalid OKR evidence"),
        evidence_ref(
            "calendar",
            "calendar://cal_over/events/evt_over",
            "Too late",
        ),
    ];

    let resolution = resolve_evidence_refs(&refs, 5);

    assert_eq!(resolution.okr_refs.len(), 1);
    assert_eq!(resolution.okr_refs[0].1.okr_id, "okr_demo");
    assert_eq!(resolution.task_refs.len(), 1);
    assert_eq!(resolution.task_refs[0].1.task_id, "task_123");
    assert_eq!(resolution.calendar_refs.len(), 1);
    assert_eq!(
        resolution.calendar_refs[0].1.source_ref(),
        "calendar://cal_1/events/evt_1"
    );
    assert!(resolution.doc_refs.is_empty());
    assert!(resolution.minutes_refs.is_empty());
    assert_eq!(resolution.degraded.len(), 3);
    assert!(resolution
        .degraded
        .iter()
        .any(|summary| summary.contains("source_ref 不是可识别的文档引用")));
    assert!(resolution
        .degraded
        .iter()
        .any(|summary| summary.contains("source_ref 不是可识别的 OKR 引用")));
    assert!(resolution
        .degraded
        .iter()
        .any(|summary| summary.contains("仅实时读取前 5 条 evidence refs")));
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
fn deduplicates_refs_before_resolving_without_echoing_duplicate_values() {
    let refs = vec![
        evidence_ref("task", " task://task_123 ", "Task evidence"),
        evidence_ref("TASK", "task://task_123", "sk-secret duplicate summary"),
        evidence_ref("task", "feishu://task/task_123", "feishu task duplicate"),
        evidence_ref(
            "okr",
            "okr://okr_demo/objectives/obj_demo/krs/kr_demo",
            "OKR evidence",
        ),
        evidence_ref(
            "okr",
            "okr:okr_demo:objective:obj_demo:kr:kr_demo",
            "OKR duplicate",
        ),
    ];

    let resolution = resolve_evidence_refs(&refs, 4);

    assert_eq!(resolution.task_refs.len(), 1);
    assert_eq!(resolution.okr_refs.len(), 1);
    assert!(resolution.calendar_refs.is_empty());
    assert_eq!(resolution.degraded.len(), 1);
    assert!(resolution.degraded[0].contains("已合并 3 条重复 evidence refs"));
    assert!(!resolution.degraded[0].contains("sk-secret"));
    assert!(!resolution.degraded[0].contains("task_123"));
    assert!(!resolution.degraded[0].contains("okr_demo"));
}

#[test]
fn resolves_doc_refs_and_deduplicates_canonical_doc_forms() {
    let refs = vec![
        evidence_ref("doc", "doc://doxcni6mOy7jLRWbEylaKKabcef", "Doc evidence"),
        evidence_ref(
            "lark_doc",
            "docx://doxcni6mOy7jLRWbEylaKKabcef",
            "Doc duplicate",
        ),
        evidence_ref("wiki", "wiki://wikcnKQ1k3p8Vabcef", "Wiki evidence"),
    ];

    let resolution = resolve_evidence_refs(&refs, 4);

    assert_eq!(resolution.doc_refs.len(), 2);
    assert_eq!(
        resolution.doc_refs[0].1.source_ref(),
        "docx://doxcni6mOy7jLRWbEylaKKabcef"
    );
    assert_eq!(
        resolution.doc_refs[1].1.source_ref(),
        "wiki://wikcnKQ1k3p8Vabcef"
    );
    assert_eq!(resolution.degraded.len(), 1);
    assert!(resolution.degraded[0].contains("已合并 1 条重复 evidence refs"));
    assert!(!resolution.degraded[0].contains("doxcni"));
}

#[test]
fn resolves_minutes_refs_and_deduplicates_canonical_forms() {
    let refs = vec![
        evidence_ref(
            "meeting",
            "minutes://obcnq3b9jl72l83w4f14xxxx",
            "Minutes evidence",
        ),
        evidence_ref(
            "lark_minutes",
            "https://sample.feishu.cn/minutes/obcnq3b9jl72l83w4f14xxxx",
            "Minutes duplicate",
        ),
    ];

    let resolution = resolve_evidence_refs(&refs, 4);

    assert_eq!(resolution.minutes_refs.len(), 1);
    assert_eq!(
        resolution.minutes_refs[0].1.source_ref(),
        "minutes://obcnq3b9jl72l83w4f14xxxx"
    );
    assert_eq!(resolution.degraded.len(), 1);
    assert!(resolution.degraded[0].contains("已合并 1 条重复 evidence refs"));
    assert!(!resolution.degraded[0].contains("obcnq3b9"));
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

#[test]
fn calendar_refs_resolve_aliases_and_deduplicate_canonical_ref() {
    let refs = vec![
        evidence_ref(
            "calendar",
            " calendar://cal_1/events/evt_1 ",
            "Calendar evidence",
        ),
        evidence_ref(
            "lark_calendar",
            "calendar://cal_1/events/evt_1",
            "duplicate summary sk-secret",
        ),
        evidence_ref(
            "feishu_calendar",
            "calendar://cal%3A2/events/evt%2F2",
            "Encoded calendar evidence",
        ),
    ];

    let resolution = resolve_evidence_refs(&refs, 4);

    assert_eq!(resolution.calendar_refs.len(), 2);
    assert_eq!(
        resolution.calendar_refs[0].1.source_ref(),
        "calendar://cal_1/events/evt_1"
    );
    assert_eq!(resolution.calendar_refs[1].1.calendar_id, "cal:2");
    assert_eq!(resolution.calendar_refs[1].1.event_id, "evt/2");
    assert_eq!(resolution.degraded.len(), 1);
    assert!(resolution.degraded[0].contains("已合并 1 条重复 evidence refs"));
    assert!(!resolution.degraded[0].contains("sk-secret"));
    assert!(!resolution.degraded[0].contains("cal_1"));
    assert!(!resolution.degraded[0].contains("evt_1"));
}

#[test]
fn invalid_calendar_refs_degrade_without_echoing_evidence_summary_or_ref() {
    let refs = vec![evidence_ref(
        "calendar",
        "calendar://sk-secret-ref/events/evt%",
        "sk-secret auth code raw transcript",
    )];

    let resolution = resolve_evidence_refs(&refs, 4);

    assert_eq!(resolution.degraded.len(), 1);
    assert!(resolution.degraded[0].contains("source_ref 不是可识别的日历引用"));
    assert!(!resolution.degraded[0].contains("sk-secret"));
    assert!(!resolution.degraded[0].contains("auth code"));
    assert!(!resolution.degraded[0].contains("raw transcript"));
}

#[test]
fn calendar_refs_require_calendar_source_type_without_cross_parsing() {
    let refs = vec![
        evidence_ref(
            "doc",
            "calendar://sk-secret-cal/events/sk-secret-event",
            "sk-secret auth code raw transcript",
        ),
        evidence_ref("calendar", "task://sk-secret-task", "task ref in calendar"),
    ];

    let resolution = resolve_evidence_refs(&refs, 4);

    assert!(resolution.calendar_refs.is_empty());
    assert_eq!(resolution.degraded.len(), 2);
    assert!(resolution
        .degraded
        .iter()
        .any(|summary| summary.contains("source_ref 不是可识别的文档引用")));
    assert!(resolution
        .degraded
        .iter()
        .any(|summary| summary.contains("source_ref 不是可识别的日历引用")));
    assert!(!resolution
        .degraded
        .iter()
        .any(|summary| summary.contains("sk-secret") || summary.contains("auth code")));
}

#[test]
fn invalid_minutes_refs_degrade_without_echoing_evidence_summary_or_ref() {
    let refs = vec![evidence_ref(
        "meeting",
        "minutes://sk-secret-token",
        "sk-secret auth code raw transcript",
    )];

    let resolution = resolve_evidence_refs(&refs, 4);

    assert!(resolution.minutes_refs.is_empty());
    assert_eq!(resolution.degraded.len(), 1);
    assert!(resolution.degraded[0].contains("source_ref 不是可识别的妙记引用"));
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

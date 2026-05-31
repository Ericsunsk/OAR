use oar_lark_adapter::{
    parse_calendar_event_source_ref, parse_doc_source_ref, parse_minutes_source_ref,
    parse_okr_kr_source_ref, parse_task_source_ref, CalendarEventSourceRef, DocSourceRef,
    MinutesSourceRef, OkrKrSourceRef,
};

pub(super) type ParsedOkrEvidenceRef = OkrKrSourceRef;
pub(super) type ParsedCalendarEvidenceRef = CalendarEventSourceRef;
pub(super) type ParsedDocEvidenceRef = DocSourceRef;
pub(super) type ParsedMinutesEvidenceRef = MinutesSourceRef;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParsedTaskEvidenceRef {
    pub(super) source_ref: String,
    pub(super) task_id: String,
}

pub(super) fn parse_okr_evidence_ref(source_ref: &str) -> Option<ParsedOkrEvidenceRef> {
    parse_okr_kr_source_ref(source_ref)
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

pub(super) fn parse_calendar_evidence_ref(source_ref: &str) -> Option<ParsedCalendarEvidenceRef> {
    parse_calendar_event_source_ref(source_ref)
}

pub(super) fn parse_doc_evidence_ref(source_ref: &str) -> Option<ParsedDocEvidenceRef> {
    parse_doc_source_ref(source_ref).ok()
}

pub(super) fn parse_minutes_evidence_ref(source_ref: &str) -> Option<ParsedMinutesEvidenceRef> {
    parse_minutes_source_ref(source_ref).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_okr_ref_supports_path_style() {
        let parsed =
            parse_okr_evidence_ref("okr://okr_demo/objectives/obj_demo/krs/kr_demo").expect("okr");
        assert_eq!(parsed.okr_id, "okr_demo");
        assert_eq!(parsed.objective_id, "obj_demo");
        assert_eq!(parsed.kr_id, "kr_demo");
    }

    #[test]
    fn parse_okr_ref_supports_colon_style() {
        let parsed =
            parse_okr_evidence_ref("okr:okr_demo:objective:obj_demo:kr:kr_demo").expect("okr");
        assert_eq!(parsed.okr_id, "okr_demo");
        assert_eq!(parsed.objective_id, "obj_demo");
        assert_eq!(parsed.kr_id, "kr_demo");
    }

    #[test]
    fn parse_okr_ref_decodes_adapter_owned_percent_encoding() {
        let parsed =
            parse_okr_evidence_ref("okr://okr%3A1/objectives/obj%2F1/krs/kr%20a%25%3F%23%3A")
                .expect("okr");
        assert_eq!(parsed.okr_id, "okr:1");
        assert_eq!(parsed.objective_id, "obj/1");
        assert_eq!(parsed.kr_id, "kr a%?#:");
    }

    #[test]
    fn parse_okr_ref_rejects_invalid_format() {
        assert!(parse_okr_evidence_ref("okr://okr_demo/objectives/obj_demo").is_none());
        assert!(parse_okr_evidence_ref("okr:okr_demo:obj:obj_demo:kr:kr_demo").is_none());
    }

    #[test]
    fn parse_okr_ref_rejects_unsafe_segments() {
        assert!(parse_okr_evidence_ref(&format!(
            "okr://{}/objectives/obj_demo/krs/kr_demo",
            "x".repeat(101)
        ))
        .is_none());
        assert!(parse_okr_evidence_ref("okr:okr?demo:objective:obj_demo:kr:kr_demo").is_none());
        assert!(parse_okr_evidence_ref("okr:okr_demo:objective:obj#demo:kr:kr_demo").is_none());
        assert!(parse_okr_evidence_ref("okr://okr demo/objectives/obj_demo/krs/kr_demo").is_none());
        assert!(
            parse_okr_evidence_ref("okr://okr_demo/objectives/obj\n_demo/krs/kr_demo").is_none()
        );
        assert!(parse_okr_evidence_ref("okr:okr_demo:objective:obj_demo:kr:kr\t_demo").is_none());
    }

    #[test]
    fn parse_task_ref_supports_task_and_feishu_task_schemes() {
        let task = parse_task_evidence_ref(" task://task_123 ").expect("task ref");
        assert_eq!(task.source_ref, "task://task_123");
        assert_eq!(task.task_id, "task_123");

        let feishu_task = parse_task_evidence_ref("feishu://task/task_456").expect("feishu task");
        assert_eq!(feishu_task.source_ref, "task://task_456");
        assert_eq!(feishu_task.task_id, "task_456");
    }

    #[test]
    fn parse_task_ref_rejects_unsafe_shapes() {
        assert!(parse_task_evidence_ref("task://").is_none());
        assert!(parse_task_evidence_ref("task://task_123/subtask").is_none());
        assert!(parse_task_evidence_ref("feishu://task/task_123?debug=true").is_none());
        assert!(
            parse_task_evidence_ref("okr://okr_demo/objectives/obj_demo/krs/kr_demo").is_none()
        );
    }

    #[test]
    fn parse_calendar_ref_supports_calendar_event_refs() {
        let calendar =
            parse_calendar_evidence_ref(" calendar://cal_1/events/evt_1 ").expect("calendar ref");
        assert_eq!(calendar.calendar_id, "cal_1");
        assert_eq!(calendar.event_id, "evt_1");
        assert_eq!(calendar.source_ref(), "calendar://cal_1/events/evt_1");
    }

    #[test]
    fn parse_calendar_ref_rejects_cross_type_and_opaque_refs() {
        assert!(parse_calendar_evidence_ref("task://task_123").is_none());
        assert!(parse_calendar_evidence_ref("calendar://customer-cadence").is_none());
        assert!(parse_calendar_evidence_ref("calendar://cal_1/event/evt_1").is_none());
        assert!(parse_calendar_evidence_ref("calendar://cal_1/events/evt?1").is_none());
    }

    #[test]
    fn parse_doc_ref_supports_docx_wiki_and_urls() {
        let doc = parse_doc_evidence_ref("docx://doxcni6mOy7jLRWbEylaKKabcef").expect("doc");
        assert_eq!(doc.source_ref(), "docx://doxcni6mOy7jLRWbEylaKKabcef");

        let wiki = parse_doc_evidence_ref("feishu://wiki/wikcnKQ1k3p8Vabcef").expect("wiki");
        assert_eq!(wiki.source_ref(), "wiki://wikcnKQ1k3p8Vabcef");

        let url = parse_doc_evidence_ref(
            "https://sample.feishu.cn/docx/doxcni6mOy7jLRWbEylaKKabcef?from=copy",
        )
        .expect("doc url");
        assert_eq!(url.source_ref(), "docx://doxcni6mOy7jLRWbEylaKKabcef");
    }

    #[test]
    fn parse_doc_ref_rejects_task_calendar_and_unsafe_refs() {
        assert!(parse_doc_evidence_ref("task://task_123").is_none());
        assert!(parse_doc_evidence_ref("calendar://cal_1/events/evt_1").is_none());
        assert!(parse_doc_evidence_ref("docx://doc?secret=true").is_none());
    }

    #[test]
    fn parse_minutes_ref_supports_minutes_and_urls() {
        let minutes =
            parse_minutes_evidence_ref("minutes://obcnq3b9jl72l83w4f14xxxx").expect("minutes");
        assert_eq!(minutes.source_ref(), "minutes://obcnq3b9jl72l83w4f14xxxx");

        let feishu = parse_minutes_evidence_ref("feishu://minutes/obcnq3b9jl72l83w4f14xxxx")
            .expect("feishu");
        assert_eq!(feishu.source_ref(), "minutes://obcnq3b9jl72l83w4f14xxxx");

        let url = parse_minutes_evidence_ref(
            "https://sample.feishu.cn/minutes/obcnq3b9jl72l83w4f14xxxx?from=copy",
        )
        .expect("url");
        assert_eq!(url.source_ref(), "minutes://obcnq3b9jl72l83w4f14xxxx");
    }

    #[test]
    fn parse_minutes_ref_rejects_cross_type_and_unsafe_refs() {
        assert!(parse_minutes_evidence_ref("task://task_123").is_none());
        assert!(parse_minutes_evidence_ref("docx://doxcni6mOy7jLRWbEylaKKabcef").is_none());
        assert!(parse_minutes_evidence_ref("minutes://enterprise-weekly-sync").is_none());
        assert!(parse_minutes_evidence_ref("minutes://obcnq3b9jl72l83w4f14xxxx/child").is_none());
    }
}

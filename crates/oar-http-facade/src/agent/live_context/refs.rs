use oar_lark_adapter::parse_task_source_ref;

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
        && trimmed
            .chars()
            .all(|character| !character.is_whitespace() && !character.is_control())
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
}

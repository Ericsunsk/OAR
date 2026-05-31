pub(super) fn is_okr_source_type(source_type: &str) -> bool {
    let source_type = source_type.trim().to_ascii_lowercase();
    source_type == "okr" || source_type == "feishu_okr" || source_type == "lark_okr"
}

pub(super) fn is_task_source_type(source_type: &str) -> bool {
    let source_type = source_type.trim().to_ascii_lowercase();
    source_type == "task" || source_type == "feishu_task" || source_type == "lark_task"
}

pub(super) fn is_calendar_source_type(source_type: &str) -> bool {
    let source_type = source_type.trim().to_ascii_lowercase();
    source_type == "calendar" || source_type == "feishu_calendar" || source_type == "lark_calendar"
}

pub(super) fn is_doc_source_type(source_type: &str) -> bool {
    let source_type = source_type.trim().to_ascii_lowercase();
    source_type == "doc"
        || source_type == "docx"
        || source_type == "wiki"
        || source_type == "feishu_doc"
        || source_type == "feishu_docx"
        || source_type == "feishu_wiki"
        || source_type == "lark_doc"
        || source_type == "lark_wiki"
}

pub(super) fn is_minutes_source_type(source_type: &str) -> bool {
    let source_type = source_type.trim().to_ascii_lowercase();
    source_type == "meeting"
        || source_type == "minutes"
        || source_type == "feishu_minutes"
        || source_type == "lark_minutes"
}

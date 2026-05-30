use crate::agent::request::AgentStreamRequest;

pub(super) fn latest_user_text(request: &AgentStreamRequest) -> Option<&str> {
    request
        .recent_messages()
        .filter(|message| message.role == "user")
        .filter_map(|message| {
            let text = message.text.trim();
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        })
        .last()
}

pub(super) fn mentions_feishu(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    text.contains("飞书")
        || contains_latin_token(&normalized, "feishu")
        || contains_latin_token(&normalized, "lark")
}

pub(super) fn asks_to_read(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    text.contains("查")
        || text.contains("看")
        || text.contains("读")
        || text.contains("有没有")
        || text.contains("是否")
        || contains_latin_token(&normalized, "show")
        || contains_latin_token(&normalized, "list")
        || contains_latin_token(&normalized, "read")
        || asks_to_count(text)
}

pub(super) fn asks_to_count(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    text.contains("几条")
        || text.contains("多少条")
        || text.contains("多少个")
        || text.contains("条数")
        || text.contains("数量")
        || text.contains("总数")
        || contains_latin_token(&normalized, "count")
        || normalized.contains("how many")
}

pub(super) fn asks_to_run_read_tool(text: &str, tool_name: &str) -> bool {
    if !text.contains(tool_name) {
        return false;
    }

    let normalized = text.to_ascii_lowercase();
    text.contains("重试")
        || text.contains("重新读取")
        || text.contains("读取")
        || text.contains("调用")
        || text.contains("运行")
        || text.contains("执行")
        || text.contains("查")
        || text.contains("看")
        || contains_latin_token(&normalized, "retry")
        || contains_latin_token(&normalized, "rerun")
        || contains_latin_token(&normalized, "read")
        || contains_latin_token(&normalized, "run")
        || contains_latin_token(&normalized, "call")
        || contains_latin_token(&normalized, "invoke")
        || contains_latin_token(&normalized, "show")
}

pub(super) fn is_self_scoped(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    let compact = text.split_whitespace().collect::<String>();
    let compact_normalized = compact.to_ascii_lowercase();
    text.contains("我的")
        || text.contains("本人")
        || text.contains("我飞书")
        || compact_normalized.contains("我okr")
        || compact_normalized.contains("我kr")
        || contains_latin_token(&normalized, "my")
}

pub(super) fn targets_non_self(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    text.contains("团队")
        || text.contains("我们")
        || text.contains("部门")
        || text.contains("别人")
        || text.contains("其他人")
        || text.contains("同事")
        || text.contains("他的")
        || text.contains("她的")
        || text.contains("他人")
        || text.contains("她人")
        || contains_latin_token(&normalized, "team")
        || contains_latin_token(&normalized, "teammate")
        || contains_latin_token(&normalized, "teammates")
        || contains_latin_token(&normalized, "department")
        || contains_latin_token(&normalized, "dept")
        || contains_latin_token(&normalized, "colleague")
        || contains_latin_token(&normalized, "colleagues")
        || contains_latin_token(&normalized, "others")
}

pub(super) fn contains_latin_token(text: &str, token: &str) -> bool {
    text.split(|character: char| !character.is_ascii_alphanumeric())
        .any(|part| part == token)
}

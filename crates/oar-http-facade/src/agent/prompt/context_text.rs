use oar_core::security::contains_sensitive_marker;

pub(super) const PROMPT_CONTEXT_TEXT_LIMIT: usize = 240;
pub(super) const REDACTED_CONTEXT_SUMMARY: &str = "已隐藏敏感摘要。";

pub(super) fn numbered_section(items: &[String], limit: usize, empty_text: &str) -> String {
    if items.is_empty() {
        return empty_text.to_string();
    }

    let summaries = items
        .iter()
        .take(limit)
        .filter_map(|summary| safe_prompt_context_text(summary))
        .enumerate()
        .map(|(index, summary)| format!("{}. {}", index + 1, summary))
        .collect::<Vec<_>>();
    if summaries.is_empty() {
        return empty_text.to_string();
    }
    summaries.join("\n")
}

pub(super) fn safe_prompt_context_text(text: &str) -> Option<String> {
    let cleaned = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() {
        return None;
    }
    if contains_sensitive_marker(&cleaned) {
        return Some(REDACTED_CONTEXT_SUMMARY.to_string());
    }
    Some(truncate_chars(&cleaned, PROMPT_CONTEXT_TEXT_LIMIT))
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut truncated = text.chars().take(max_chars).collect::<String>();
    truncated.push_str("...");
    truncated
}

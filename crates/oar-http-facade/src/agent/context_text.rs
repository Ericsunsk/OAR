use std::collections::HashSet;

use oar_core::security::contains_sensitive_marker;

pub(in crate::agent) const PROMPT_CONTEXT_TEXT_LIMIT: usize = 240;
pub(in crate::agent) const REDACTED_CONTEXT_SUMMARY: &str = "已隐藏敏感摘要。";

pub(in crate::agent) fn numbered_section(
    items: &[String],
    limit: usize,
    empty_text: &str,
) -> String {
    let summaries = safe_context_summaries(items, limit);
    if summaries.is_empty() {
        return empty_text.to_string();
    }
    summaries
        .into_iter()
        .enumerate()
        .map(|(index, summary)| format!("{}. {}", index + 1, summary))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(in crate::agent) fn safe_context_summaries(items: &[String], limit: usize) -> Vec<String> {
    let mut seen_summaries = HashSet::new();
    items
        .iter()
        .filter_map(|summary| safe_prompt_context_text(summary))
        .filter(|summary| seen_summaries.insert(prompt_context_text_key(summary)))
        .take(limit)
        .collect()
}

pub(in crate::agent) fn safe_prompt_context_text(text: &str) -> Option<String> {
    let cleaned = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() {
        return None;
    }
    if contains_sensitive_marker(&cleaned) {
        return Some(REDACTED_CONTEXT_SUMMARY.to_string());
    }
    Some(truncate_chars(&cleaned, PROMPT_CONTEXT_TEXT_LIMIT))
}

fn prompt_context_text_key(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut truncated = text.chars().take(max_chars).collect::<String>();
    truncated.push_str("...");
    truncated
}

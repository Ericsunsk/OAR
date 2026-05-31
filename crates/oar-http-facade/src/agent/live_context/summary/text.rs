const LIVE_SUMMARY_CHAR_LIMIT: usize = 200;

pub(in crate::agent::live_context) fn finalize_summary(value: String) -> String {
    truncate_chars(&value, LIVE_SUMMARY_CHAR_LIMIT)
}

pub(in crate::agent::live_context) fn compact_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(in crate::agent::live_context) fn truncate_chars(value: &str, limit: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= limit {
        return value.to_string();
    }
    let mut truncated = value
        .chars()
        .take(limit.saturating_sub(1))
        .collect::<String>();
    truncated.push('…');
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_text_collapses_whitespace_without_echoing_layout() {
        assert_eq!(compact_text("  hello\n\tworld  "), "hello world");
        assert_eq!(compact_text(" \n\t "), "");
    }

    #[test]
    fn truncate_chars_preserves_multibyte_boundary() {
        assert_eq!(truncate_chars("你好世界", 3), "你好…");
        assert_eq!(truncate_chars("你好", 3), "你好");
    }

    #[test]
    fn finalize_summary_applies_global_char_limit() {
        let summary = finalize_summary("一".repeat(250));

        assert_eq!(summary.chars().count(), LIVE_SUMMARY_CHAR_LIMIT);
        assert!(summary.ends_with('…'));
    }
}

pub(super) fn sanitized_text_or(value: Option<&str>, fallback: &str) -> String {
    let candidate = value.unwrap_or("").trim();
    let candidate = if candidate.is_empty() {
        fallback.trim()
    } else {
        candidate
    };
    let compact = candidate
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect::<String>();
    let compact = compact.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() > 320 {
        compact.chars().take(320).collect()
    } else {
        compact
    }
}

pub(super) fn safe_ledger_text(value: &str, fallback: &str) -> String {
    let sanitized = sanitized_text_or(Some(value), fallback);
    if oar_core::security::contains_sensitive_marker(&sanitized) {
        fallback.to_string()
    } else {
        sanitized
    }
}

pub(super) fn safe_correlation_key(value: &str) -> String {
    let sanitized = sanitized_text_or(Some(value), "redacted");
    let safe_shape = !sanitized.is_empty()
        && sanitized.chars().count() <= 160
        && sanitized
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, ':' | '_' | '-' | '.' | '/'));
    if safe_shape && !oar_core::security::contains_sensitive_marker(&sanitized) {
        sanitized
    } else {
        "redacted".to_string()
    }
}

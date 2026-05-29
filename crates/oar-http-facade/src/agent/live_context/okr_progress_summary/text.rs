use super::super::summary::{compact_text, truncate_chars};

const TITLE_LIMIT: usize = 20;
const VALUE_LIMIT: usize = 24;

pub(super) fn compact(value: &str) -> String {
    compact_text(value)
}

pub(super) fn non_empty_compact(value: Option<&str>) -> Option<String> {
    value
        .map(compact_text)
        .filter(|value| !value.trim().is_empty())
}

pub(super) fn short_title(value: &str) -> Option<String> {
    let value = compact_text(value);
    if value.is_empty() {
        None
    } else {
        Some(truncate_chars(&value, TITLE_LIMIT))
    }
}

pub(super) fn short_value(value: &str) -> Option<String> {
    let value = compact_text(value);
    if value.is_empty() {
        None
    } else {
        Some(truncate_chars(&value, VALUE_LIMIT))
    }
}

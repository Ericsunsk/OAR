use serde::de::Deserializer;
use serde::Deserialize;
use serde_json::Value;

pub(super) fn deserialize_option_stringish<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(value.and_then(stringify_json_scalar))
}

pub(super) fn deserialize_option_content_value<'de, D>(
    deserializer: D,
) -> Result<Option<Value>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(value.and_then(normalize_content_value))
}

fn normalize_content_value(value: Value) -> Option<Value> {
    match value {
        Value::Null => None,
        Value::String(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                None
            } else if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
                Some(parsed)
            } else {
                Some(Value::String(trimmed.to_string()))
            }
        }
        other => Some(other),
    }
}

fn stringify_json_scalar(value: Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(v) => non_empty(v),
        Value::Number(v) => Some(v.to_string()),
        Value::Bool(v) => Some(v.to_string()),
        Value::Array(_) | Value::Object(_) => None,
    }
}

pub(super) fn content_value_to_text(value: &Value) -> Option<String> {
    let mut parts = Vec::new();
    collect_content_text(value, 0, &mut parts);
    if parts.is_empty() {
        return stringify_json_scalar(value.clone());
    }
    non_empty(parts.join("\n")).map(limit_text)
}

fn collect_content_text(value: &Value, depth: usize, parts: &mut Vec<String>) {
    const MAX_CONTENT_DEPTH: usize = 8;
    const MAX_CONTENT_PARTS: usize = 64;
    if depth > MAX_CONTENT_DEPTH || parts.len() >= MAX_CONTENT_PARTS {
        return;
    }

    match value {
        Value::Null => {}
        Value::String(value) => {
            if let Some(value) = non_empty(value.clone()) {
                parts.push(value);
            }
        }
        Value::Number(value) => parts.push(value.to_string()),
        Value::Bool(value) => parts.push(value.to_string()),
        Value::Array(values) => {
            for value in values
                .iter()
                .take(MAX_CONTENT_PARTS.saturating_sub(parts.len()))
            {
                collect_content_text(value, depth + 1, parts);
                if parts.len() >= MAX_CONTENT_PARTS {
                    break;
                }
            }
        }
        Value::Object(map) => {
            for field in [
                "text",
                "plain_text",
                "content",
                "title",
                "name",
                "value",
                "text_run",
                "paragraph",
                "elements",
                "children",
                "blocks",
            ] {
                if let Some(value) = map.get(field) {
                    collect_content_text(value, depth + 1, parts);
                    if parts.len() >= MAX_CONTENT_PARTS {
                        break;
                    }
                }
            }
        }
    }
}

pub(super) fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn limit_text(value: String) -> String {
    const MAX_CONTENT_TEXT_CHARS: usize = 4096;
    if value.chars().count() <= MAX_CONTENT_TEXT_CHARS {
        return value;
    }
    let mut truncated = value
        .chars()
        .take(MAX_CONTENT_TEXT_CHARS.saturating_sub(3))
        .collect::<String>();
    truncated.push_str("...");
    truncated
}

pub(super) fn latest_updated_time(values: &[Option<&str>]) -> Option<String> {
    let numeric_latest = values
        .iter()
        .flatten()
        .filter_map(|value| {
            let trimmed = value.trim();
            trimmed.parse::<u64>().ok().map(|parsed| (parsed, trimmed))
        })
        .max_by_key(|(parsed, _)| *parsed)
        .map(|(_, raw)| raw.to_string());

    numeric_latest.or_else(|| {
        values
            .iter()
            .flatten()
            .map(|value| value.trim())
            .find(|value| !value.is_empty())
            .map(ToString::to_string)
    })
}

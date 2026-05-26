use serde_json::Value;

const DRY_RUN_PREFIX: &str = "=== Dry Run ===";

pub fn strip_dry_run_prefix(raw: &str) -> &str {
    raw.trim_start()
        .strip_prefix(DRY_RUN_PREFIX)
        .map(str::trim_start)
        .unwrap_or(raw.trim_start())
}

pub fn parse_cli_json(raw: &str) -> Result<Value, serde_json::Error> {
    serde_json::from_str(strip_dry_run_prefix(raw))
}

pub fn progress_list_entries(value: &Value) -> Vec<&Value> {
    value
        .get("data")
        .and_then(|data| data.get("progress_list"))
        .and_then(Value::as_array)
        .map(|items| items.iter().collect())
        .unwrap_or_default()
}

pub fn normalize_cycle_detail_content(value: &mut Value) {
    let Some(objectives) = value
        .get_mut("data")
        .and_then(|data| data.get_mut("objectives"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };

    for objective in objectives {
        normalize_content_field(objective, "content");

        if let Some(key_results) = objective.get_mut("key_results").and_then(Value::as_array_mut) {
            for key_result in key_results {
                normalize_content_field(key_result, "content");
            }
        }
    }
}

fn normalize_content_field(node: &mut Value, field_name: &str) {
    let Some(raw_content) = node
        .get(field_name)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
    else {
        return;
    };

    if let Ok(parsed) = serde_json::from_str::<Value>(&raw_content) {
        if let Some(obj) = node.as_object_mut() {
            obj.insert(field_name.to_string(), parsed);
        }
    }
}

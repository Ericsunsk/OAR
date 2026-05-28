pub(crate) fn non_empty_env(env: &impl Fn(&str) -> Option<String>, key: &str) -> Option<String> {
    env(key).and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

use oar_core::action::capability::FeishuScope;

pub(super) const MINUTES_READONLY_SCOPE_COMPAT: &str = "minutes:minutes:readonly";

pub(super) fn has_okr_evidence_read_scopes(scopes: &[String]) -> bool {
    has_feishu_scope(scopes, FeishuScope::OkrContentRead)
        && has_feishu_scope(scopes, FeishuScope::OkrProgressRead)
}

pub(super) fn has_task_read_scope(scopes: &[String]) -> bool {
    has_feishu_scope(scopes, FeishuScope::TaskRead)
}

pub(super) fn has_calendar_evidence_read_scopes(scopes: &[String]) -> bool {
    has_feishu_scope(scopes, FeishuScope::CalendarRead)
        && has_feishu_scope(scopes, FeishuScope::CalendarEventRead)
}

pub(super) fn has_minutes_basic_read_scope(scopes: &[String]) -> bool {
    has_feishu_scope(scopes, FeishuScope::MinutesBasicRead)
        || scopes
            .iter()
            .any(|scope| scope.trim() == MINUTES_READONLY_SCOPE_COMPAT)
}

pub(super) fn missing_feishu_scope_names<'a>(
    scopes: &[String],
    required_scopes: &'a [&'static str],
) -> Vec<&'a str> {
    required_scopes
        .iter()
        .filter_map(|required| {
            if scopes.iter().any(|scope| scope.trim() == *required) {
                None
            } else {
                Some(*required)
            }
        })
        .collect()
}

pub(super) fn has_feishu_scope(scopes: &[String], required: FeishuScope) -> bool {
    let required = required.as_str();
    scopes.iter().any(|scope| scope.trim() == required)
}

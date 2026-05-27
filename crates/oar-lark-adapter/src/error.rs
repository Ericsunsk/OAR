#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeishuRefreshFailureClass {
    ReauthRequired,
    ConfigRequired,
    Transient,
}

pub fn classify_feishu_refresh_failure(code: i64, _http_status: u16) -> FeishuRefreshFailureClass {
    match code {
        20037 | 20064 | 20073 => FeishuRefreshFailureClass::ReauthRequired,
        20074 => FeishuRefreshFailureClass::ConfigRequired,
        20050 | 20072 => FeishuRefreshFailureClass::Transient,
        _ => FeishuRefreshFailureClass::Transient,
    }
}

pub fn safe_error_for_failure_class(class: FeishuRefreshFailureClass) -> &'static str {
    match class {
        FeishuRefreshFailureClass::ReauthRequired => "invalid_grant",
        FeishuRefreshFailureClass::ConfigRequired => "refresh_config_required",
        FeishuRefreshFailureClass::Transient => "temporarily unavailable",
    }
}

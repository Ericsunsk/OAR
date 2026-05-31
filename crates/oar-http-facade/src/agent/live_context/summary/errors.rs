use oar_lark_adapter::{FeishuCalendarReadError, FeishuOkrReadError, FeishuTaskReadError};

pub(in crate::agent::live_context) fn task_read_error_reason(
    error: FeishuTaskReadError,
) -> &'static str {
    match error {
        FeishuTaskReadError::InvalidSourceRef => "任务引用无效",
        FeishuTaskReadError::Unauthorized => "授权已失效，需要重新登录",
        FeishuTaskReadError::Forbidden => "授权缺少任务读取权限",
        FeishuTaskReadError::NotFound => "任务不存在或无权访问",
        FeishuTaskReadError::UpstreamClient => "任务读取请求被拒绝",
        FeishuTaskReadError::UpstreamTransient
        | FeishuTaskReadError::Transport
        | FeishuTaskReadError::ApiFailure => "任务实时读取暂不可用",
        FeishuTaskReadError::OversizedResponse | FeishuTaskReadError::InvalidJson => {
            "任务实时读取返回不可用"
        }
    }
}

pub(in crate::agent::live_context) fn calendar_read_error_reason(
    error: FeishuCalendarReadError,
) -> &'static str {
    match error {
        FeishuCalendarReadError::InvalidRequest => "日历读取请求无效",
        FeishuCalendarReadError::Unauthorized => "授权已失效，需要重新登录",
        FeishuCalendarReadError::Forbidden => "授权缺少日历读取权限",
        FeishuCalendarReadError::NotFound => "日历目标不存在或无权访问",
        FeishuCalendarReadError::UpstreamClient => "日历读取请求被拒绝",
        FeishuCalendarReadError::UpstreamTransient
        | FeishuCalendarReadError::Transport
        | FeishuCalendarReadError::ApiFailure => "日历实时读取暂不可用",
        FeishuCalendarReadError::OversizedResponse | FeishuCalendarReadError::InvalidJson => {
            "日历实时读取返回不可用"
        }
    }
}

pub(in crate::agent::live_context) fn okr_read_error_reason(
    error: FeishuOkrReadError,
) -> &'static str {
    match error {
        FeishuOkrReadError::InvalidRequest => "OKR 读取请求无效",
        FeishuOkrReadError::Unauthorized => "授权已失效，需要重新登录",
        FeishuOkrReadError::Forbidden => "授权缺少 OKR 读取权限",
        FeishuOkrReadError::UpstreamClient => "OKR 读取请求被拒绝",
        FeishuOkrReadError::UpstreamTransient
        | FeishuOkrReadError::Transport
        | FeishuOkrReadError::ApiFailure => "OKR 实时读取暂不可用",
        FeishuOkrReadError::OversizedResponse | FeishuOkrReadError::InvalidJson => {
            "OKR 实时读取返回不可用"
        }
    }
}

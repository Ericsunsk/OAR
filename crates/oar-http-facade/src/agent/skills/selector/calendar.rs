use crate::agent::request::AgentStreamRequest;
use crate::agent::tools::AgentReadTool;

use super::common::{
    asks_to_run_read_tool, contains_latin_token, is_self_scoped, latest_user_text, mentions_feishu,
    targets_non_self,
};

pub(super) fn latest_user_requests_feishu_calendar_free_busy(request: &AgentStreamRequest) -> bool {
    let Some(latest_user_text) = latest_user_text(request) else {
        return false;
    };

    latest_user_has_explicit_self_calendar_free_busy_intent(latest_user_text)
}

fn latest_user_has_explicit_self_calendar_free_busy_intent(text: &str) -> bool {
    latest_user_requests_calendar_free_busy_tool(text)
        || ((mentions_feishu(text) || mentions_calendar(text))
            && mentions_calendar_free_busy(text)
            && is_self_scoped(text)
            && !targets_non_self(text)
            && !asks_calendar_write(text)
            && !asks_calendar_event_listing(text))
}

fn latest_user_requests_calendar_free_busy_tool(text: &str) -> bool {
    asks_to_run_read_tool(text, AgentReadTool::CalendarFreeBusy.spec().name)
        && !targets_non_self(text)
}

fn mentions_calendar(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    text.contains("日历")
        || text.contains("日程")
        || contains_latin_token(&normalized, "calendar")
        || contains_latin_token(&normalized, "cal")
}

fn mentions_calendar_free_busy(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    text.contains("忙闲")
        || text.contains("空闲")
        || text.contains("有空")
        || text.contains("没空")
        || text.contains("能不能开会")
        || text.contains("能否开会")
        || text.contains("可用时间")
        || normalized.contains("free-busy")
        || normalized.contains("freebusy")
        || contains_latin_token(&normalized, "availability")
        || contains_latin_token(&normalized, "available")
        || contains_latin_token(&normalized, "busy")
        || contains_latin_token(&normalized, "free")
}

fn asks_calendar_write(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    text.contains("创建")
        || text.contains("新建")
        || text.contains("添加")
        || text.contains("新增")
        || text.contains("更新")
        || text.contains("修改")
        || text.contains("删除")
        || text.contains("预约")
        || text.contains("预订")
        || text.contains("安排")
        || text.contains("邀请")
        || text.contains("约会")
        || contains_latin_token(&normalized, "create")
        || contains_latin_token(&normalized, "add")
        || contains_latin_token(&normalized, "update")
        || contains_latin_token(&normalized, "delete")
        || contains_latin_token(&normalized, "book")
        || contains_latin_token(&normalized, "invite")
        || contains_latin_token(&normalized, "schedule")
}

fn asks_calendar_event_listing(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    text.contains("日程列表")
        || text.contains("会议列表")
        || text.contains("有哪些日程")
        || text.contains("有哪些会议")
        || text.contains("日程安排")
        || text.contains("会议安排")
        || normalized.contains("event list")
        || normalized.contains("meeting list")
        || normalized.contains("my schedule")
}

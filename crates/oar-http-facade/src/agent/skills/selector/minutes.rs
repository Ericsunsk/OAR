use crate::agent::request::AgentStreamRequest;
use crate::agent::tools::AgentReadTool;

use super::common::{
    asks_to_count, asks_to_read, asks_to_run_read_tool, contains_latin_token, is_self_scoped,
    latest_user_text, mentions_feishu, targets_non_self,
};

pub(super) fn latest_user_requests_feishu_minutes_summary(request: &AgentStreamRequest) -> bool {
    let Some(latest_user_text) = latest_user_text(request) else {
        return false;
    };

    latest_user_has_explicit_self_minutes_read_intent(latest_user_text)
}

pub(super) fn latest_user_has_explicit_self_minutes_read_intent(text: &str) -> bool {
    latest_user_requests_minutes_summary_tool(text)
        || (mentions_minutes(text)
            && mentions_self_or_recent(text)
            && !targets_non_self(text)
            && !asks_minutes_unsupported_operation(text)
            && (asks_to_read(text) || asks_to_count(text) || mentions_recent(text)))
}

pub(super) fn mentions_minutes(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    text.contains("妙记")
        || text.contains("会议纪要")
        || text.contains("会议记录")
        || normalized.contains("meeting notes")
        || normalized.contains("meeting note")
        || contains_latin_token(&normalized, "minutes")
}

fn latest_user_requests_minutes_summary_tool(text: &str) -> bool {
    asks_to_run_read_tool(text, AgentReadTool::MinutesSummary.spec().name)
        && !targets_non_self(text)
        && !asks_minutes_unsupported_operation(text)
}

fn mentions_self_or_recent(text: &str) -> bool {
    is_self_scoped(text) || mentions_recent(text) || mentions_feishu(text)
}

fn mentions_recent(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    text.contains("最近")
        || text.contains("近期")
        || contains_latin_token(&normalized, "recent")
        || contains_latin_token(&normalized, "latest")
}

fn asks_minutes_unsupported_operation(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    text.contains("创建")
        || text.contains("新建")
        || text.contains("上传")
        || text.contains("删除")
        || text.contains("分享")
        || text.contains("导出")
        || text.contains("下载")
        || text.contains("逐字稿")
        || text.contains("转写")
        || text.contains("录音")
        || text.contains("视频")
        || contains_latin_token(&normalized, "create")
        || contains_latin_token(&normalized, "upload")
        || contains_latin_token(&normalized, "delete")
        || contains_latin_token(&normalized, "share")
        || contains_latin_token(&normalized, "export")
        || contains_latin_token(&normalized, "download")
        || contains_latin_token(&normalized, "transcript")
        || contains_latin_token(&normalized, "transcripts")
        || contains_latin_token(&normalized, "media")
        || contains_latin_token(&normalized, "recording")
        || contains_latin_token(&normalized, "video")
}

use crate::agent::request::AgentStreamRequest;
use crate::agent::tools::AgentReadTool;

use super::common::{
    asks_to_read, asks_to_run_read_tool, contains_latin_token, is_self_scoped, latest_user_text,
    targets_non_self,
};

pub(super) fn latest_user_requests_feishu_task_summary(request: &AgentStreamRequest) -> bool {
    let Some(latest_user_text) = latest_user_text(request) else {
        return false;
    };

    latest_user_has_explicit_self_task_read_intent(latest_user_text)
}

fn latest_user_has_explicit_self_task_read_intent(text: &str) -> bool {
    latest_user_requests_task_summary_tool(text)
        || (mentions_task(text)
            && asks_to_read(text)
            && is_self_scoped(text)
            && !targets_non_self(text)
            && !asks_task_write(text))
}

fn latest_user_requests_task_summary_tool(text: &str) -> bool {
    asks_to_run_read_tool(text, AgentReadTool::TaskSummary.spec().name) && !targets_non_self(text)
}

fn mentions_task(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    text.contains("任务")
        || text.contains("待办")
        || text.contains("我负责")
        || contains_latin_token(&normalized, "task")
        || contains_latin_token(&normalized, "tasks")
        || contains_latin_token(&normalized, "todo")
        || contains_latin_token(&normalized, "todos")
}

fn asks_task_write(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    text.contains("创建")
        || text.contains("新建")
        || text.contains("添加")
        || text.contains("新增")
        || text.contains("更新")
        || text.contains("修改")
        || text.contains("删除")
        || text.contains("完成")
        || text.contains("关闭")
        || text.contains("指派")
        || text.contains("分配")
        || text.contains("评论")
        || contains_latin_token(&normalized, "create")
        || contains_latin_token(&normalized, "add")
        || contains_latin_token(&normalized, "update")
        || contains_latin_token(&normalized, "change")
        || contains_latin_token(&normalized, "set")
        || contains_latin_token(&normalized, "delete")
        || contains_latin_token(&normalized, "complete")
        || contains_latin_token(&normalized, "assign")
        || contains_latin_token(&normalized, "comment")
}

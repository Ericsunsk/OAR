use std::collections::BTreeMap;

use oar_lark_adapter::{
    AsyncFeishuTaskRead, FeishuTaskListRequest, FeishuTaskReadClient, ReqwestAsyncHttpClient,
    SecretString, TaskListType, TaskReadSummary, TaskUserIdType,
};

use super::summary::{compact_text, finalize_summary, truncate_chars};

const MY_TASK_PAGE_SIZE: u16 = 100;
const MY_TASK_PAGE_LIMIT: usize = 3;
const MY_TASK_TITLE_LIMIT: usize = 4;

pub(super) async fn read_my_task_summary(
    task_client: &mut FeishuTaskReadClient<ReqwestAsyncHttpClient>,
    access_token: SecretString,
) -> Result<String, oar_lark_adapter::FeishuTaskReadError> {
    let mut tasks = Vec::new();
    let mut page_token = None;
    let mut has_more = false;

    for _ in 0..MY_TASK_PAGE_LIMIT {
        let page = task_client
            .list_task_summaries(FeishuTaskListRequest {
                user_access_token: access_token.clone(),
                page_size: Some(MY_TASK_PAGE_SIZE),
                page_token,
                completed: None,
                task_type: TaskListType::MyTasks,
                user_id_type: TaskUserIdType::OpenId,
            })
            .await?;
        tasks.extend(page.tasks);
        has_more = page.has_more;
        page_token = page.page_token;
        if !has_more || page_token.is_none() {
            break;
        }
    }

    if tasks.is_empty() {
        return Ok("工具 feishu.task.summarize_my_tasks｜实时：未读取到我负责的任务。".to_string());
    }

    let mut status_counts = BTreeMap::new();
    for task in &tasks {
        let status = task
            .status
            .as_deref()
            .map(compact_text)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "unknown".to_string());
        *status_counts
            .entry(truncate_chars(&status, 24))
            .or_insert(0_usize) += 1;
    }
    let status_summary = status_counts
        .into_iter()
        .map(|(status, count)| format!("{status} {count}"))
        .collect::<Vec<_>>()
        .join("、");
    let examples = tasks
        .iter()
        .filter_map(task_title)
        .take(MY_TASK_TITLE_LIMIT)
        .collect::<Vec<_>>();
    let examples_suffix = if examples.is_empty() {
        String::new()
    } else {
        format!("；示例：{}", examples.join(" / "))
    };
    let more_suffix = if has_more {
        format!("；已按上限读取前 {} 页，仍可能有更多", MY_TASK_PAGE_LIMIT)
    } else {
        String::new()
    };

    Ok(finalize_summary(format!(
        "工具 feishu.task.summarize_my_tasks｜实时：读取到 {} 条我负责的任务；状态：{}{}{}。",
        tasks.len(),
        status_summary,
        examples_suffix,
        more_suffix
    )))
}

fn task_title(task: &TaskReadSummary) -> Option<String> {
    task.title
        .as_deref()
        .map(compact_text)
        .filter(|title| !title.is_empty())
        .map(|title| truncate_chars(&title, 24))
}

#[cfg(test)]
mod tests {
    use super::*;
    use oar_lark_adapter::{TaskReadDue, TaskReadOwner};

    #[test]
    fn task_title_is_compact_and_limited() {
        let task = TaskReadSummary {
            source_ref: "task://task_1".to_string(),
            task_id: "task_1".to_string(),
            title: Some("  very   long task title that should be compacted  ".to_string()),
            status: Some("open".to_string()),
            due: Some(TaskReadDue {
                timestamp: Some("1780000000000".to_string()),
                is_all_day: Some(true),
            }),
            owners: vec![TaskReadOwner {
                owner_id: Some("ou_sensitive".to_string()),
                owner_type: Some("open_id".to_string()),
            }],
            update_time: None,
        };

        assert_eq!(
            task_title(&task).as_deref(),
            Some("very long task title th…")
        );
    }
}

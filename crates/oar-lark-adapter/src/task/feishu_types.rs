use serde::Deserialize;
use serde_json::Value;

use super::source_ref::{valid_task_id, TaskSourceRef};
use super::types::{TaskReadDue, TaskReadOwner, TaskReadPage, TaskReadSummary};

#[derive(Debug, Deserialize)]
pub(super) struct FeishuTaskGetResponse {
    pub code: i64,
    pub data: Option<FeishuTaskGetData>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FeishuTaskGetData {
    pub task: Option<FeishuTask>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FeishuTaskListResponse {
    pub code: i64,
    pub data: Option<FeishuTaskListData>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FeishuTaskListData {
    #[serde(default, alias = "tasks")]
    pub items: Vec<FeishuTask>,
    #[serde(default)]
    pub has_more: bool,
    pub page_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FeishuTask {
    #[serde(alias = "guid", alias = "task_id")]
    pub id: Option<String>,
    #[serde(alias = "summary", alias = "name")]
    pub title: Option<String>,
    #[serde(default)]
    pub status: Option<Value>,
    #[serde(default)]
    pub completed: Option<bool>,
    #[serde(default)]
    pub completed_at: Option<Value>,
    #[serde(default)]
    pub due: Option<Value>,
    #[serde(default)]
    pub members: Vec<FeishuTaskMember>,
    #[serde(default)]
    pub owner: Option<FeishuTaskMember>,
    #[serde(default)]
    pub creator: Option<FeishuTaskMember>,
    #[serde(default, alias = "updated_at")]
    pub update_time: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct FeishuTaskMember {
    #[serde(
        alias = "member_id",
        alias = "open_id",
        alias = "user_id",
        alias = "id"
    )]
    pub id: Option<String>,
    #[serde(alias = "member_type", alias = "user_id_type", alias = "type")]
    pub member_type: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
}

impl TaskReadSummary {
    pub(super) fn from_feishu_task(source_ref: &TaskSourceRef, task: FeishuTask) -> Self {
        let status = task_status(&task);
        let due = task.due.as_ref().and_then(task_due);
        let owners = task_owners(&task);
        let update_time = task.update_time.as_ref().and_then(stringify_json_scalar);
        Self {
            source_ref: source_ref.source_ref(),
            task_id: task.id.unwrap_or_else(|| source_ref.task_id.clone()),
            title: non_empty(task.title),
            status,
            due,
            owners,
            update_time,
        }
    }
}

impl TaskReadPage {
    pub(super) fn from_feishu_list(data: FeishuTaskListData) -> Self {
        let tasks = data
            .items
            .into_iter()
            .filter_map(|task| {
                let task_id = task.id.as_deref()?.trim().to_string();
                if !valid_task_id(&task_id) {
                    return None;
                }
                Some(TaskReadSummary::from_feishu_task(
                    &TaskSourceRef { task_id },
                    task,
                ))
            })
            .collect::<Vec<_>>();
        Self {
            tasks,
            has_more: data.has_more,
            page_token: non_empty(data.page_token),
        }
    }
}

fn task_status(task: &FeishuTask) -> Option<String> {
    task.status
        .as_ref()
        .and_then(stringify_json_scalar)
        .or_else(|| {
            task.completed
                .map(|completed| status_for_completed(completed).to_string())
        })
        .or_else(|| {
            task.completed_at
                .as_ref()
                .and_then(stringify_json_scalar)
                .filter(|value| value.trim() != "0" && !value.trim().is_empty())
                .map(|_| "completed".to_string())
        })
}

fn status_for_completed(completed: bool) -> &'static str {
    if completed {
        "completed"
    } else {
        "open"
    }
}

fn task_due(value: &Value) -> Option<TaskReadDue> {
    match value {
        Value::Object(map) => {
            let timestamp = ["timestamp", "time", "date"]
                .iter()
                .find_map(|field| map.get(*field).and_then(stringify_json_scalar));
            let is_all_day = map.get("is_all_day").and_then(Value::as_bool);
            if timestamp.is_some() || is_all_day.is_some() {
                Some(TaskReadDue {
                    timestamp,
                    is_all_day,
                })
            } else {
                None
            }
        }
        _ => stringify_json_scalar(value).map(|timestamp| TaskReadDue {
            timestamp: Some(timestamp),
            is_all_day: None,
        }),
    }
}

fn task_owners(task: &FeishuTask) -> Vec<TaskReadOwner> {
    let mut owners = task
        .members
        .iter()
        .filter(|member| {
            member
                .role
                .as_deref()
                .map(|role| matches!(role, "assignee" | "owner" | "OWNER" | "ASSIGNEE"))
                .unwrap_or(false)
        })
        .map(TaskReadOwner::from)
        .collect::<Vec<_>>();

    if owners.is_empty() {
        if let Some(owner) = task.owner.as_ref() {
            owners.push(TaskReadOwner::from(owner));
        } else if let Some(creator) = task.creator.as_ref() {
            owners.push(TaskReadOwner::from(creator));
        }
    }

    owners
}

impl From<&FeishuTaskMember> for TaskReadOwner {
    fn from(value: &FeishuTaskMember) -> Self {
        Self {
            owner_id: value.id.clone(),
            owner_type: value.member_type.clone(),
        }
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn stringify_json_scalar(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(v) => non_empty(Some(v.clone())),
        Value::Number(v) => Some(v.to_string()),
        Value::Bool(v) => Some(v.to_string()),
        Value::Array(_) | Value::Object(_) => None,
    }
}

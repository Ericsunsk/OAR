use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::redaction::SecretString;

#[derive(Clone, PartialEq, Eq)]
pub struct FeishuTaskGetRequest {
    pub user_access_token: SecretString,
    pub source_ref: String,
    pub user_id_type: TaskUserIdType,
}

impl fmt::Debug for FeishuTaskGetRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuTaskGetRequest")
            .field("user_access_token", &"[REDACTED]")
            .field("source_ref", &self.source_ref)
            .field("user_id_type", &self.user_id_type)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct FeishuTaskListRequest {
    pub user_access_token: SecretString,
    pub page_size: Option<u16>,
    pub page_token: Option<String>,
    pub completed: Option<bool>,
    pub task_type: TaskListType,
    pub user_id_type: TaskUserIdType,
}

impl fmt::Debug for FeishuTaskListRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuTaskListRequest")
            .field("user_access_token", &"[REDACTED]")
            .field("page_size", &self.page_size)
            .field("page_token", &self.page_token)
            .field("completed", &self.completed)
            .field("task_type", &self.task_type)
            .field("user_id_type", &self.user_id_type)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskUserIdType {
    OpenId,
    UserId,
    UnionId,
}

impl TaskUserIdType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskUserIdType::OpenId => "open_id",
            TaskUserIdType::UserId => "user_id",
            TaskUserIdType::UnionId => "union_id",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskListType {
    MyTasks,
}

impl TaskListType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskListType::MyTasks => "my_tasks",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSourceRef {
    pub task_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct TaskReadSummary {
    pub source_ref: String,
    pub task_id: String,
    pub title: Option<String>,
    pub status: Option<String>,
    pub due: Option<TaskReadDue>,
    #[serde(default)]
    pub owners: Vec<TaskReadOwner>,
    pub update_time: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct TaskReadDue {
    pub timestamp: Option<String>,
    pub is_all_day: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct TaskReadOwner {
    pub owner_id: Option<String>,
    pub owner_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct TaskReadPage {
    pub tasks: Vec<TaskReadSummary>,
    pub has_more: bool,
    pub page_token: Option<String>,
}

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
            source_ref: format!("task://{}", source_ref.task_id),
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

pub(super) fn valid_task_id(task_id: &str) -> bool {
    !task_id.is_empty()
        && task_id.len() <= 100
        && !task_id.contains('/')
        && !task_id.contains('?')
        && !task_id.contains('#')
        && task_id
            .chars()
            .all(|character| !character.is_whitespace() && !character.is_control())
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

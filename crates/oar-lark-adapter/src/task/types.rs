use std::fmt;

use serde::{Deserialize, Serialize};

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

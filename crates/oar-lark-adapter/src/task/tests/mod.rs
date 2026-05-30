use crate::redaction::SecretString;
use crate::task::{FeishuTaskGetRequest, FeishuTaskListRequest, TaskListType, TaskUserIdType};

fn sample_request() -> FeishuTaskGetRequest {
    FeishuTaskGetRequest {
        user_access_token: SecretString::new("u-very-secret-task-token"),
        source_ref: "task://task_123".to_string(),
        user_id_type: TaskUserIdType::OpenId,
    }
}

fn sample_list_request() -> FeishuTaskListRequest {
    FeishuTaskListRequest {
        user_access_token: SecretString::new("u-very-secret-task-token"),
        page_size: Some(2),
        page_token: None,
        completed: Some(false),
        task_type: TaskListType::MyTasks,
        user_id_type: TaskUserIdType::OpenId,
    }
}

mod errors;
mod get;
mod list;
mod requests;

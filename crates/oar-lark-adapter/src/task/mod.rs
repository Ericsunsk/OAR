mod client;
mod error;
mod source_ref;
mod types;

pub use client::{
    build_get_task_request, build_list_tasks_request, AsyncFeishuTaskRead, FeishuTaskReadClient,
};
pub use error::FeishuTaskReadError;
pub use source_ref::{parse_task_source_ref, TaskSourceRef};
pub use types::{
    FeishuTaskGetRequest, FeishuTaskListRequest, TaskListType, TaskReadDue, TaskReadOwner,
    TaskReadPage, TaskReadSummary, TaskUserIdType,
};

#[cfg(test)]
mod tests;

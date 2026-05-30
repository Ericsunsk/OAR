#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSourceRef {
    pub task_id: String,
}

impl TaskSourceRef {
    pub fn source_ref(&self) -> String {
        format!("task://{}", self.task_id)
    }
}

pub fn parse_task_source_ref(
    source_ref: &str,
) -> Result<TaskSourceRef, super::error::FeishuTaskReadError> {
    let trimmed = source_ref.trim();
    let task_id = if let Some(task_id) = trimmed.strip_prefix("task://") {
        task_id
    } else if let Some(task_id) = trimmed.strip_prefix("feishu://task/") {
        task_id
    } else {
        return Err(super::error::FeishuTaskReadError::InvalidSourceRef);
    };
    if !valid_task_id(task_id) {
        return Err(super::error::FeishuTaskReadError::InvalidSourceRef);
    }
    Ok(TaskSourceRef {
        task_id: task_id.to_string(),
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::FeishuTaskReadError;

    #[test]
    fn source_ref_parser_accepts_task_and_feishu_task_schemes() {
        let parsed = parse_task_source_ref(" task://task_123 ").expect("source ref");
        assert_eq!(parsed.task_id, "task_123");
        assert_eq!(parsed.source_ref(), "task://task_123");

        let feishu = parse_task_source_ref("feishu://task/task_456").expect("source ref");
        assert_eq!(feishu.task_id, "task_456");
        assert_eq!(feishu.source_ref(), "task://task_456");

        assert_eq!(
            parse_task_source_ref("okr://okr_1"),
            Err(FeishuTaskReadError::InvalidSourceRef)
        );
        assert_eq!(
            parse_task_source_ref("task://"),
            Err(FeishuTaskReadError::InvalidSourceRef)
        );
        assert_eq!(
            parse_task_source_ref("task://task_123/subtask"),
            Err(FeishuTaskReadError::InvalidSourceRef)
        );
    }

    #[test]
    fn valid_task_id_rejects_unsafe_shapes() {
        assert!(!valid_task_id(""));
        assert!(!valid_task_id("task/123"));
        assert!(!valid_task_id("task?123"));
        assert!(!valid_task_id("task#123"));
        assert!(!valid_task_id("task 123"));
        assert!(!valid_task_id("task\n123"));
        assert!(!valid_task_id(&"x".repeat(101)));
    }
}

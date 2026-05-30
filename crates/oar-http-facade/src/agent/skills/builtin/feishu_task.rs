use crate::agent::tools::AgentReadTool;

pub(in crate::agent::skills) const ID: &str = "feishu.task";
pub(in crate::agent::skills) const DISPLAY_NAME: &str = "Feishu Task";
pub(in crate::agent::skills) const PURPOSE: &str =
    "理解用户关于本人飞书任务、待办、我负责事项、数量和状态概览的查询。";
pub(in crate::agent::skills) const SAFETY: &str = "Skill 只描述领域能力；真实读取必须由后端 tool runtime 校验 OAuth scope 后通过 Lark adapter 执行。只读工具可自动执行，写操作必须 dry-run 和人工确认。";
pub(in crate::agent::skills) const MANIFEST_MARKDOWN: &str = include_str!("feishu_task/SKILL.md");

pub(in crate::agent::skills) const TOOLS: &[AgentReadTool] = &[AgentReadTool::TaskSummary];

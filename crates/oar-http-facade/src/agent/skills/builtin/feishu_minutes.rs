use crate::agent::tools::AgentReadTool;

pub(in crate::agent::skills) const ID: &str = "feishu.minutes";
pub(in crate::agent::skills) const DISPLAY_NAME: &str = "Feishu Minutes";
pub(in crate::agent::skills) const PURPOSE: &str =
    "理解用户关于本人飞书妙记、最近妙记和 meeting notes 的只读查询。";
pub(in crate::agent::skills) const SAFETY: &str = "Skill 只描述领域能力；真实读取必须由后端 tool runtime 校验 OAuth scope 后通过 Lark adapter 执行。只读工具可自动执行，逐字稿导出、媒体下载、上传、删除、分享等操作必须走独立能力并经过 dry-run 和人工确认。";
pub(in crate::agent::skills) const MANIFEST_MARKDOWN: &str =
    include_str!("feishu_minutes/SKILL.md");

pub(in crate::agent::skills) const TOOLS: &[AgentReadTool] = &[AgentReadTool::MinutesSummary];

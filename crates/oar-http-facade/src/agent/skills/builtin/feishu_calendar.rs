use crate::agent::tools::AgentReadTool;

pub(in crate::agent::skills) const ID: &str = "feishu.calendar";
pub(in crate::agent::skills) const DISPLAY_NAME: &str = "Feishu Calendar";
pub(in crate::agent::skills) const PURPOSE: &str =
    "理解用户关于本人飞书日历忙闲、空闲时间、availability 和受限日程摘要的只读查询。";
pub(in crate::agent::skills) const SAFETY: &str = "Skill 只描述领域能力；真实读取必须由后端 tool runtime 校验 OAuth scope 后通过 Lark adapter 执行。只读忙闲工具可自动执行，建会、邀请、通知等写操作必须 dry-run 和人工确认。";
pub(in crate::agent::skills) const MANIFEST_MARKDOWN: &str =
    include_str!("feishu_calendar/SKILL.md");

pub(in crate::agent::skills) const TOOLS: &[AgentReadTool] = &[
    AgentReadTool::CalendarFreeBusy,
    AgentReadTool::CalendarEvents,
];

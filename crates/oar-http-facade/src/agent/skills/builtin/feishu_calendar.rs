use super::super::catalog::AgentSkillToolSpec;

pub(in crate::agent::skills) const ID: &str = "feishu.calendar";
pub(in crate::agent::skills) const DISPLAY_NAME: &str = "Feishu Calendar";
pub(in crate::agent::skills) const PURPOSE: &str =
    "理解用户关于本人飞书日历忙闲、空闲时间和 availability 概览的只读查询。";
pub(in crate::agent::skills) const SAFETY: &str = "Skill 只描述领域能力；真实读取必须由后端 tool runtime 校验 OAuth scope 后通过 Lark adapter 执行。只读忙闲工具可自动执行，建会、邀请、通知等写操作必须 dry-run 和人工确认。";
pub(in crate::agent::skills) const MANIFEST_MARKDOWN: &str =
    include_str!("feishu_calendar/SKILL.md");

pub(in crate::agent::skills) const TOOLS: &[AgentSkillToolSpec] = &[AgentSkillToolSpec {
    name: "feishu.calendar.summarize_my_free_busy",
    description: "后端只读汇总当前用户未来 7 天的 Feishu 主日历忙闲时段数量和示例窗口。",
}];

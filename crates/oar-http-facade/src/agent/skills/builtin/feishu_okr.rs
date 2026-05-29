use super::super::catalog::AgentSkillToolSpec;

pub(in crate::agent::skills) const ID: &str = "feishu.okr";
pub(in crate::agent::skills) const DISPLAY_NAME: &str = "Feishu OKR";
pub(in crate::agent::skills) const PURPOSE: &str =
    "理解用户关于本人飞书 OKR、目标、关键结果、数量和内容概览的查询。";
pub(in crate::agent::skills) const SAFETY: &str = "Skill 只描述领域能力；真实读取必须由后端 tool runtime 校验 OAuth scope 后通过 Lark adapter 执行。只读工具可自动执行，写操作必须 dry-run 和人工确认。";
pub(in crate::agent::skills) const MANIFEST_MARKDOWN: &str = include_str!("feishu_okr/SKILL.md");

pub(in crate::agent::skills) const TOOLS: &[AgentSkillToolSpec] = &[AgentSkillToolSpec {
    name: "feishu.okr.summarize_my_okr",
    description: "后端只读汇总当前用户的 Feishu OKR 周期、Objective 和 KR 数量。",
}];

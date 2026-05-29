use super::builtin::{feishu_calendar, feishu_okr, feishu_task};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::agent) enum AgentSkill {
    FeishuCalendar,
    FeishuOkr,
    FeishuTask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::agent) struct AgentSkillSpec {
    pub(in crate::agent) id: &'static str,
    pub(in crate::agent) display_name: &'static str,
    pub(in crate::agent) purpose: &'static str,
    pub(in crate::agent) tools: &'static [AgentSkillToolSpec],
    pub(in crate::agent) safety: &'static str,
    pub(in crate::agent) manifest_markdown: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::agent) struct AgentSkillToolSpec {
    pub(in crate::agent) name: &'static str,
    pub(in crate::agent) description: &'static str,
}

impl AgentSkill {
    pub(in crate::agent) const fn spec(self) -> AgentSkillSpec {
        match self {
            Self::FeishuCalendar => AgentSkillSpec {
                id: feishu_calendar::ID,
                display_name: feishu_calendar::DISPLAY_NAME,
                purpose: feishu_calendar::PURPOSE,
                tools: feishu_calendar::TOOLS,
                safety: feishu_calendar::SAFETY,
                manifest_markdown: feishu_calendar::MANIFEST_MARKDOWN,
            },
            Self::FeishuOkr => AgentSkillSpec {
                id: feishu_okr::ID,
                display_name: feishu_okr::DISPLAY_NAME,
                purpose: feishu_okr::PURPOSE,
                tools: feishu_okr::TOOLS,
                safety: feishu_okr::SAFETY,
                manifest_markdown: feishu_okr::MANIFEST_MARKDOWN,
            },
            Self::FeishuTask => AgentSkillSpec {
                id: feishu_task::ID,
                display_name: feishu_task::DISPLAY_NAME,
                purpose: feishu_task::PURPOSE,
                tools: feishu_task::TOOLS,
                safety: feishu_task::SAFETY,
                manifest_markdown: feishu_task::MANIFEST_MARKDOWN,
            },
        }
    }

    pub(in crate::agent) fn prompt_summary(self) -> String {
        let spec = self.spec();
        let tools = spec
            .tools
            .iter()
            .map(|tool| format!("{}（{}）", tool.name, tool.description))
            .collect::<Vec<_>>()
            .join("；");
        format!(
            "{}｜{}｜用途：{}｜可用后端工具：{}｜安全：{}",
            spec.id, spec.display_name, spec.purpose, tools, spec.safety
        )
    }
}

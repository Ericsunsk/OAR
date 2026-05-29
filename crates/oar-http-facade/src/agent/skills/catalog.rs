use super::builtin::{feishu_calendar, feishu_okr, feishu_task};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::agent) enum AgentSkill {
    Calendar,
    Okr,
    Task,
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
            Self::Calendar => AgentSkillSpec {
                id: feishu_calendar::ID,
                display_name: feishu_calendar::DISPLAY_NAME,
                purpose: feishu_calendar::PURPOSE,
                tools: feishu_calendar::TOOLS,
                safety: feishu_calendar::SAFETY,
                manifest_markdown: feishu_calendar::MANIFEST_MARKDOWN,
            },
            Self::Okr => AgentSkillSpec {
                id: feishu_okr::ID,
                display_name: feishu_okr::DISPLAY_NAME,
                purpose: feishu_okr::PURPOSE,
                tools: feishu_okr::TOOLS,
                safety: feishu_okr::SAFETY,
                manifest_markdown: feishu_okr::MANIFEST_MARKDOWN,
            },
            Self::Task => AgentSkillSpec {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::tools::AgentReadTool;

    #[test]
    fn builtin_skill_specs_expose_expected_manifests_and_tools() {
        assert_feishu_okr_spec(AgentSkill::Okr.spec());
        assert_feishu_task_spec(AgentSkill::Task.spec());
        assert_feishu_calendar_spec(AgentSkill::Calendar.spec());
    }

    fn assert_feishu_okr_spec(spec: AgentSkillSpec) {
        assert_eq!(spec.id, "feishu.okr");
        assert_eq!(spec.display_name, "Feishu OKR");
        assert_eq!(spec.tools.len(), 2);
        assert_skill_tools_registered(spec);
        assert_eq!(spec.tools[0].name, "feishu.okr.summarize_my_okr");
        assert_eq!(spec.tools[1].name, "feishu.okr.summarize_my_progress");
        assert!(spec.safety.contains("后端 tool runtime"));
        assert!(spec.manifest_markdown.contains("## Activation"));
        assert!(spec
            .manifest_markdown
            .contains("feishu.okr.summarize_my_okr"));
        assert!(spec
            .manifest_markdown
            .contains("feishu.okr.summarize_my_progress"));
    }

    fn assert_feishu_task_spec(spec: AgentSkillSpec) {
        assert_eq!(spec.id, "feishu.task");
        assert_eq!(spec.display_name, "Feishu Task");
        assert!(spec.purpose.contains("飞书任务"));
        assert_eq!(spec.tools.len(), 1);
        assert_skill_tools_registered(spec);
        assert_eq!(spec.tools[0].name, "feishu.task.summarize_my_tasks");
        assert!(spec.tools[0].description.contains("只读汇总"));
        assert!(spec.manifest_markdown.contains("# Feishu Task"));
    }

    fn assert_feishu_calendar_spec(spec: AgentSkillSpec) {
        assert_eq!(spec.id, "feishu.calendar");
        assert_eq!(spec.display_name, "Feishu Calendar");
        assert!(spec.purpose.contains("忙闲"));
        assert_eq!(spec.tools.len(), 1);
        assert_skill_tools_registered(spec);
        assert_eq!(spec.tools[0].name, "feishu.calendar.summarize_my_free_busy");
        assert!(spec.tools[0].description.contains("未来 7 天"));
        assert!(spec.manifest_markdown.contains("# Feishu Calendar"));
    }

    fn assert_skill_tools_registered(spec: AgentSkillSpec) {
        for skill_tool in spec.tools {
            let registered = AgentReadTool::from_name(skill_tool.name)
                .expect("builtin skill tool must be registered");
            assert_eq!(registered.spec().name, skill_tool.name);
            assert!(!skill_tool.description.trim().is_empty());
        }
    }
}

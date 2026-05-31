use oar_core::action::capability::{
    try_feishu_scopes_for_action_types, CapabilityActionType, CapabilityScopeDerivationError,
    FeishuScope,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::agent) enum AgentReadTool {
    CalendarEvents,
    CalendarFreeBusy,
    MinutesSummary,
    OkrSummary,
    OkrProgress,
    TaskSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::agent) struct AgentToolSpec {
    pub(in crate::agent) name: &'static str,
    pub(in crate::agent) description: &'static str,
    pub(in crate::agent) required_action_types: &'static [CapabilityActionType],
    pub(in crate::agent) effect: AgentToolEffect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::agent) enum AgentToolEffect {
    Read,
}

const FEISHU_OKR_SUMMARIZE_MY_OKR_ACTION_TYPES: &[CapabilityActionType] = &[
    CapabilityActionType::OkrPeriodRead,
    CapabilityActionType::OkrContentRead,
];
const FEISHU_OKR_SUMMARIZE_MY_PROGRESS_ACTION_TYPES: &[CapabilityActionType] = &[
    CapabilityActionType::OkrPeriodRead,
    CapabilityActionType::OkrContentRead,
    CapabilityActionType::OkrProgressRead,
];
const FEISHU_TASK_SUMMARIZE_MY_TASKS_ACTION_TYPES: &[CapabilityActionType] =
    &[CapabilityActionType::TaskRead];
const FEISHU_CALENDAR_SUMMARIZE_MY_EVENTS_ACTION_TYPES: &[CapabilityActionType] = &[
    CapabilityActionType::CalendarRead,
    CapabilityActionType::CalendarEventRead,
];
const FEISHU_CALENDAR_SUMMARIZE_MY_FREE_BUSY_ACTION_TYPES: &[CapabilityActionType] =
    &[CapabilityActionType::CalendarFreeBusyRead];
const FEISHU_MINUTES_SUMMARIZE_MY_MINUTES_ACTION_TYPES: &[CapabilityActionType] =
    &[CapabilityActionType::MinutesSearchRead];

impl AgentReadTool {
    #[cfg(test)]
    pub(in crate::agent) fn from_name(name: &str) -> Option<Self> {
        match name {
            "feishu.calendar.summarize_my_events" => Some(Self::CalendarEvents),
            "feishu.calendar.summarize_my_free_busy" => Some(Self::CalendarFreeBusy),
            "feishu.minutes.summarize_my_minutes" => Some(Self::MinutesSummary),
            "feishu.okr.summarize_my_okr" => Some(Self::OkrSummary),
            "feishu.okr.summarize_my_progress" => Some(Self::OkrProgress),
            "feishu.task.summarize_my_tasks" => Some(Self::TaskSummary),
            _ => None,
        }
    }

    pub(in crate::agent) const fn spec(self) -> AgentToolSpec {
        match self {
            Self::CalendarEvents => AgentToolSpec {
                name: "feishu.calendar.summarize_my_events",
                description: "只读汇总当前用户未来 7 天 Feishu 主日历日程实例的受限摘要。",
                required_action_types: FEISHU_CALENDAR_SUMMARIZE_MY_EVENTS_ACTION_TYPES,
                effect: AgentToolEffect::Read,
            },
            Self::CalendarFreeBusy => AgentToolSpec {
                name: "feishu.calendar.summarize_my_free_busy",
                description: "只读汇总当前用户未来 7 天的 Feishu 主日历忙闲时段。",
                required_action_types: FEISHU_CALENDAR_SUMMARIZE_MY_FREE_BUSY_ACTION_TYPES,
                effect: AgentToolEffect::Read,
            },
            Self::MinutesSummary => AgentToolSpec {
                name: "feishu.minutes.summarize_my_minutes",
                description: "只读汇总当前用户的 Feishu 妙记/meeting notes 数量和安全元信息示例。",
                required_action_types: FEISHU_MINUTES_SUMMARIZE_MY_MINUTES_ACTION_TYPES,
                effect: AgentToolEffect::Read,
            },
            Self::OkrSummary => AgentToolSpec {
                name: "feishu.okr.summarize_my_okr",
                description: "只读汇总当前用户的 Feishu OKR 周期、Objective 和 KR 数量。",
                required_action_types: FEISHU_OKR_SUMMARIZE_MY_OKR_ACTION_TYPES,
                effect: AgentToolEffect::Read,
            },
            Self::OkrProgress => AgentToolSpec {
                name: "feishu.okr.summarize_my_progress",
                description: "只读汇总当前用户的 Feishu OKR 进展、最近更新、延期和风险信号。",
                required_action_types: FEISHU_OKR_SUMMARIZE_MY_PROGRESS_ACTION_TYPES,
                effect: AgentToolEffect::Read,
            },
            Self::TaskSummary => AgentToolSpec {
                name: "feishu.task.summarize_my_tasks",
                description: "只读汇总当前用户在 Feishu 中我负责的任务数量、状态和示例标题。",
                required_action_types: FEISHU_TASK_SUMMARIZE_MY_TASKS_ACTION_TYPES,
                effect: AgentToolEffect::Read,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::agent) enum AgentToolSpecError {
    CapabilityNotRegistered(CapabilityActionType),
}

impl AgentToolSpecError {
    pub(in crate::agent) fn safe_reason(self) -> String {
        match self {
            Self::CapabilityNotRegistered(action_type) => {
                format!("工具能力未注册：{}", action_type.as_str())
            }
        }
    }
}

impl AgentToolSpec {
    pub(in crate::agent) fn required_feishu_scopes(
        self,
    ) -> Result<Vec<FeishuScope>, AgentToolSpecError> {
        try_feishu_scopes_for_action_types(self.required_action_types).map_err(
            |error| match error {
                CapabilityScopeDerivationError::CapabilityNotRegistered(action_type) => {
                    AgentToolSpecError::CapabilityNotRegistered(action_type)
                }
            },
        )
    }

    pub(in crate::agent) fn required_feishu_scope_names(
        self,
    ) -> Result<Vec<&'static str>, AgentToolSpecError> {
        self.required_feishu_scopes()
            .map(|scopes| scopes.into_iter().map(FeishuScope::as_str).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oar_core::action::capability::{
        find_by_action_type, CapabilityEffect, CapabilityExecutionMode,
    };

    #[test]
    fn read_tool_manifest_derives_feishu_scopes_from_core_capability_matrix() {
        let spec = AgentReadTool::OkrSummary.spec();

        assert_eq!(
            spec.required_action_types,
            &[
                CapabilityActionType::OkrPeriodRead,
                CapabilityActionType::OkrContentRead
            ]
        );
        assert_eq!(
            spec.required_feishu_scopes().expect("scopes"),
            vec![FeishuScope::OkrPeriodRead, FeishuScope::OkrContentRead]
        );

        let spec = AgentReadTool::OkrProgress.spec();
        assert_eq!(
            spec.required_action_types,
            &[
                CapabilityActionType::OkrPeriodRead,
                CapabilityActionType::OkrContentRead,
                CapabilityActionType::OkrProgressRead
            ]
        );
        assert_eq!(
            spec.required_feishu_scopes().expect("scopes"),
            vec![
                FeishuScope::OkrPeriodRead,
                FeishuScope::OkrContentRead,
                FeishuScope::OkrProgressRead
            ]
        );

        let spec = AgentReadTool::TaskSummary.spec();
        assert_eq!(
            spec.required_action_types,
            &[CapabilityActionType::TaskRead]
        );
        assert_eq!(
            spec.required_feishu_scopes().expect("scopes"),
            vec![FeishuScope::TaskRead]
        );

        let spec = AgentReadTool::CalendarEvents.spec();
        assert_eq!(
            spec.required_action_types,
            &[
                CapabilityActionType::CalendarRead,
                CapabilityActionType::CalendarEventRead
            ]
        );
        assert_eq!(
            spec.required_feishu_scopes().expect("scopes"),
            vec![FeishuScope::CalendarRead, FeishuScope::CalendarEventRead]
        );

        let spec = AgentReadTool::CalendarFreeBusy.spec();
        assert_eq!(
            spec.required_action_types,
            &[CapabilityActionType::CalendarFreeBusyRead]
        );
        assert_eq!(
            spec.required_feishu_scopes().expect("scopes"),
            vec![FeishuScope::CalendarFreeBusyRead]
        );

        let spec = AgentReadTool::MinutesSummary.spec();
        assert_eq!(
            spec.required_action_types,
            &[CapabilityActionType::MinutesSearchRead]
        );
        assert_eq!(
            spec.required_feishu_scopes().expect("scopes"),
            vec![FeishuScope::MinutesSearchRead]
        );
    }

    #[test]
    fn read_tool_manifest_only_uses_auto_read_capabilities() {
        for tool in [
            AgentReadTool::CalendarEvents,
            AgentReadTool::CalendarFreeBusy,
            AgentReadTool::MinutesSummary,
            AgentReadTool::OkrSummary,
            AgentReadTool::OkrProgress,
            AgentReadTool::TaskSummary,
        ] {
            let spec = tool.spec();
            assert_eq!(spec.effect, AgentToolEffect::Read);
            for action_type in spec.required_action_types {
                let capability =
                    find_by_action_type(*action_type).expect("read tool capability is registered");
                assert_eq!(capability.effect, CapabilityEffect::Read);
                assert_eq!(capability.execution_mode, CapabilityExecutionMode::AutoRead);
                assert!(!capability.is_write());
                assert!(!capability.enters_execution_allowlist());
            }
        }
    }
}

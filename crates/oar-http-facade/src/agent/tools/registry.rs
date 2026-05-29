use oar_core::action::capability::{
    try_feishu_scopes_for_action_types, CapabilityActionType, CapabilityScopeDerivationError,
    FeishuScope,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::agent) enum AgentReadTool {
    FeishuOkrSummarizeMyOkr,
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

impl AgentReadTool {
    pub(in crate::agent) const fn spec(self) -> AgentToolSpec {
        match self {
            Self::FeishuOkrSummarizeMyOkr => AgentToolSpec {
                name: "feishu.okr.summarize_my_okr",
                description: "只读汇总当前用户的 Feishu OKR 周期、Objective 和 KR 数量。",
                required_action_types: FEISHU_OKR_SUMMARIZE_MY_OKR_ACTION_TYPES,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_tool_manifest_derives_feishu_scopes_from_core_capability_matrix() {
        let spec = AgentReadTool::FeishuOkrSummarizeMyOkr.spec();

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
    }
}

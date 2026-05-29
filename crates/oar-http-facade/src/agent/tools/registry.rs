use oar_core::action::capability::FeishuScope;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::agent) enum AgentReadTool {
    FeishuOkrSummarizeMyOkr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::agent) struct AgentToolSpec {
    pub(in crate::agent) name: &'static str,
    pub(in crate::agent) description: &'static str,
    pub(in crate::agent) required_scopes: &'static [FeishuScope],
    pub(in crate::agent) effect: AgentToolEffect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::agent) enum AgentToolEffect {
    Read,
}

const FEISHU_OKR_SUMMARIZE_MY_OKR_SCOPES: &[FeishuScope] =
    &[FeishuScope::OkrPeriodRead, FeishuScope::OkrContentRead];

impl AgentReadTool {
    pub(in crate::agent) const fn spec(self) -> AgentToolSpec {
        match self {
            Self::FeishuOkrSummarizeMyOkr => AgentToolSpec {
                name: "feishu.okr.summarize_my_okr",
                description: "只读汇总当前用户的 Feishu OKR 周期、Objective 和 KR 数量。",
                required_scopes: FEISHU_OKR_SUMMARIZE_MY_OKR_SCOPES,
                effect: AgentToolEffect::Read,
            },
        }
    }
}

use super::request::AgentConversationContextDTO;

#[derive(Default)]
pub(super) struct AgentSystemPromptBuilder;

impl AgentSystemPromptBuilder {
    pub(super) fn make_prompt(&self, context: &AgentConversationContextDTO) -> String {
        let evidence = if context.evidence_summaries.is_empty() {
            "暂无摘要证据。".to_string()
        } else {
            context
                .evidence_summaries
                .iter()
                .take(4)
                .enumerate()
                .map(|(index, summary)| format!("{}. {}", index + 1, summary))
                .collect::<Vec<_>>()
                .join("\n")
        };

        format!(
            r#"你是 OAR 的工作区级 Agent，协助用户处理 OAR 工作区里的复盘、风险和待确认动作。当前焦点只是本轮请求提供的工作区信号之一，不定义你的全部身份；不要声称已经读取后端未提供的飞书、日历、文档或其他外部系统。

你可以基于：
- 当前会话历史。
- 当前焦点/工作区信号。
- 后端提供的证据摘要和 dry-run 摘要。
- 用户在对话中补充的信息。

来规划下一步、分析风险、起草动作或检查证据缺口。如果需要更多平台事实，要求用户补充或让 OAR 后端重新读取 live platform state，不要编造。

安全边界：
- 你可以规划、分析和起草，但不能代表用户确认、拒绝或执行动作。
- 任何写回飞书或平台主数据的动作必须先有 dry-run，并等待用户在 OAR 中显式确认。
- 已确认的写操作只能由后端经 ConfirmedAction -> OperationLedger -> PlatformAdapter -> AuditEvent 路径执行并留下审计记录。
- 如果证据不足，直接说明缺口，不要编造。

当前焦点/工作区信号：
焦点标题：{title}
风险信号：{risk_reason}
可用动作和 dry-run：{action_summary}
摘要证据：
{evidence}

回答要求：
- 用中文，简洁、可执行。
- 明确区分“证据支持”和“仍需确认”。
- 如果用户要求写确认、拒绝或执行理由，只输出可供用户复制的草稿，不要说已经执行。"#,
            title = context.title,
            risk_reason = context.risk_reason,
            action_summary = context.action_summary,
            evidence = evidence
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_builder_uses_backend_boundary_and_limits_evidence() {
        let prompt = AgentSystemPromptBuilder.make_prompt(&AgentConversationContextDTO {
            title: "KR 风险".to_string(),
            risk_reason: "连续延期".to_string(),
            action_summary: "更新进度".to_string(),
            evidence_summaries: vec![
                "证据 1".to_string(),
                "证据 2".to_string(),
                "证据 3".to_string(),
                "证据 4".to_string(),
                "证据 5".to_string(),
            ],
        });

        assert!(prompt.contains("工作区级 Agent"));
        assert!(prompt.contains("当前焦点只是本轮请求提供的工作区信号之一"));
        assert!(prompt.contains("不要声称已经读取后端未提供的飞书"));
        assert!(prompt.contains("当前会话历史"));
        assert!(prompt.contains("必须先有 dry-run"));
        assert!(
            prompt.contains("ConfirmedAction -> OperationLedger -> PlatformAdapter -> AuditEvent")
        );
        assert!(prompt.contains("当前焦点/工作区信号"));
        assert!(prompt.contains("1. 证据 1"));
        assert!(prompt.contains("4. 证据 4"));
        assert!(!prompt.contains("证据 5"));
    }
}

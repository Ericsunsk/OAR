use super::request::AgentConversationContextDTO;

const EVIDENCE_SUMMARY_LIMIT: usize = 4;
const WORKSPACE_SECTION_LIMIT: usize = 5;

#[derive(Default)]
pub(super) struct AgentSystemPromptBuilder;

impl AgentSystemPromptBuilder {
    pub(super) fn make_prompt(&self, context: &AgentConversationContextDTO) -> String {
        let evidence = numbered_section(
            &context.evidence_summaries,
            EVIDENCE_SUMMARY_LIMIT,
            "暂无摘要证据。",
        );
        let workspace_summary = if context.workspace_summary.trim().is_empty() {
            "暂无工作区摘要。"
        } else {
            context.workspace_summary.trim()
        };
        let workspace_signals = numbered_section(
            &context.workspace_signals,
            WORKSPACE_SECTION_LIMIT,
            "暂无工作区信号摘要。",
        );
        let pending_actions = numbered_section(
            &context.pending_action_summaries,
            WORKSPACE_SECTION_LIMIT,
            "暂无待处理动作摘要。",
        );

        format!(
            r#"你是 OAR 的工作区级 Agent，协助用户处理 OAR 工作区里的复盘、风险和待确认动作。当前焦点只是本轮请求提供的工作区信号之一，不定义你的全部身份；不要声称已经读取后端未提供的飞书、日历、文档或其他外部系统。

你可以基于：
- 当前会话历史。
- 当前焦点/工作区信号。
- 后端/前端提供的工作区摘要、证据摘要和 dry-run 摘要。
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

后端/前端提供的工作区摘要：
{workspace_summary}
Top 工作区信号：
{workspace_signals}
待处理动作摘要：
{pending_actions}

回答要求：
- 用中文，简洁、可执行。
- 明确区分“证据支持”和“仍需确认”。
- 如果用户要求写确认、拒绝或执行理由，只输出可供用户复制的草稿，不要说已经执行。"#,
            title = context.title,
            risk_reason = context.risk_reason,
            action_summary = context.action_summary,
            evidence = evidence,
            workspace_summary = workspace_summary,
            workspace_signals = workspace_signals,
            pending_actions = pending_actions
        )
    }
}

fn numbered_section(items: &[String], limit: usize, empty_text: &str) -> String {
    if items.is_empty() {
        return empty_text.to_string();
    }

    items
        .iter()
        .take(limit)
        .enumerate()
        .map(|(index, summary)| format!("{}. {}", index + 1, summary))
        .collect::<Vec<_>>()
        .join("\n")
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
            workspace_summary: "工作区摘要：共 6 个风险，严重/高 4 个。".to_string(),
            workspace_signals: vec![
                "信号 1".to_string(),
                "信号 2".to_string(),
                "信号 3".to_string(),
                "信号 4".to_string(),
                "信号 5".to_string(),
                "信号 6".to_string(),
            ],
            pending_action_summaries: vec![
                "动作 1".to_string(),
                "动作 2".to_string(),
                "动作 3".to_string(),
                "动作 4".to_string(),
                "动作 5".to_string(),
                "动作 6".to_string(),
            ],
        });

        assert!(prompt.contains("工作区级 Agent"));
        assert!(prompt.contains("当前焦点只是本轮请求提供的工作区信号之一"));
        assert!(prompt.contains("不要声称已经读取后端未提供的飞书"));
        assert!(prompt.contains("当前会话历史"));
        assert!(prompt.contains("后端/前端提供的工作区摘要、证据摘要和 dry-run 摘要"));
        assert!(prompt.contains("必须先有 dry-run"));
        assert!(
            prompt.contains("ConfirmedAction -> OperationLedger -> PlatformAdapter -> AuditEvent")
        );
        assert!(prompt.contains("当前焦点/工作区信号"));
        assert!(prompt.contains("1. 证据 1"));
        assert!(prompt.contains("4. 证据 4"));
        assert!(!prompt.contains("证据 5"));
        assert!(prompt.contains("工作区摘要：共 6 个风险，严重/高 4 个。"));
        assert!(prompt.contains("1. 信号 1"));
        assert!(prompt.contains("5. 信号 5"));
        assert!(!prompt.contains("信号 6"));
        assert!(prompt.contains("1. 动作 1"));
        assert!(prompt.contains("5. 动作 5"));
        assert!(!prompt.contains("动作 6"));
    }
}

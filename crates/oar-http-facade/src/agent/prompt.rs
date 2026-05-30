use super::request::AgentConversationContextDTO;

mod context_text;

use context_text::{numbered_section, safe_prompt_context_text};

const EVIDENCE_SUMMARY_LIMIT: usize = 4;
const WORKSPACE_SECTION_LIMIT: usize = 5;
const LEDGER_EVENT_SECTION_LIMIT: usize = 5;
const LIVE_FEISHU_SECTION_LIMIT: usize = 4;
const ACTIVATED_SKILL_SECTION_LIMIT: usize = 4;

#[derive(Default)]
pub(super) struct AgentSystemPromptBuilder;

impl AgentSystemPromptBuilder {
    pub(super) fn make_prompt(context: &AgentConversationContextDTO) -> String {
        let title =
            safe_prompt_context_text(&context.title).unwrap_or_else(|| "未选择风险".to_string());
        let risk_reason = safe_prompt_context_text(&context.risk_reason)
            .unwrap_or_else(|| "暂无风险说明。".to_string());
        let action_summary = safe_prompt_context_text(&context.action_summary)
            .unwrap_or_else(|| "暂无建议动作。".to_string());
        let evidence = numbered_section(
            &context.evidence_summaries,
            EVIDENCE_SUMMARY_LIMIT,
            "暂无摘要证据。",
        );
        let workspace_summary = safe_prompt_context_text(&context.workspace_summary)
            .unwrap_or_else(|| "暂无工作区摘要。".to_string());
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
        let ledger_events = numbered_section(
            &context.ledger_event_summaries,
            LEDGER_EVENT_SECTION_LIMIT,
            "暂无 review-inbox 审计摘要。",
        );
        let live_feishu = numbered_section(
            &context.live_feishu_read_summaries,
            LIVE_FEISHU_SECTION_LIMIT,
            "暂无实时 Feishu 读取结果。",
        );
        let activated_skills = numbered_section(
            &context.activated_skill_summaries,
            ACTIVATED_SKILL_SECTION_LIMIT,
            "本轮没有激活内置 skill。",
        );

        format!(
            r#"你是 OAR 的工作区级 Agent，协助用户处理 OAR 工作区里的复盘、风险和待确认动作。当前焦点只是本轮请求提供的工作区信号之一，不定义你的全部身份；不要声称已经读取后端未提供的飞书、日历、文档或其他外部系统。

你可以基于：
- 当前会话历史。
- 当前焦点/工作区信号。
- 后端/前端提供的工作区摘要、证据摘要、dry-run 摘要和 review-inbox 安全审计摘要。
- 已激活内置 skill 的领域说明和后端工具说明。
- 后端 tool result 提供的只读实时 Feishu 读取结果。
- 用户在对话中补充的信息。

来规划下一步、分析风险、起草动作或检查证据缺口。review-inbox 审计摘要是后端/前端提供的安全 OperationLedger 摘要，只能说明已有提议、dry-run、确认或审计状态；它不是实时 Feishu 读取，不授权写执行，也不证明平台主数据已经改变。内置 skill 只是领域和工具说明，不代表你能直接调用飞书；真实平台读取只能来自后端 tool runtime/live context。实时 Feishu 读取结果只能来自后端 tool result 或后端 live context，不要把自己的推断当成实时读取。日历忙闲摘要只代表忙碌窗口，不代表完整日程详情、标题、参会人或会议内容；日程摘要只代表后端返回的受限 agenda 摘要，不代表完整日历记录、描述、会议链接、附件或完整参会人清单。如果需要更多平台事实，要求用户补充或让 OAR 后端重新读取 live platform state，不要编造。

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
Review-inbox 安全审计摘要：
{ledger_events}

已激活内置 skill：
{activated_skills}

实时 Feishu 读取结果（来自后端 tool result；可能由 evidence refs live read 或只读 tool runtime 产生）：
{live_feishu}

回答要求：
- 用中文，简洁、可执行。
- 明确区分“前端/既有摘要 / review-inbox 审计摘要”和“后端实时读取结果”。
- 明确区分“证据支持”和“仍需确认”。
- 如果用户要求写确认、拒绝或执行理由，只输出可供用户复制的草稿，不要说已经执行。"#,
            title = title,
            risk_reason = risk_reason,
            action_summary = action_summary,
            evidence = evidence,
            workspace_summary = workspace_summary,
            workspace_signals = workspace_signals,
            pending_actions = pending_actions,
            ledger_events = ledger_events,
            activated_skills = activated_skills,
            live_feishu = live_feishu
        )
    }
}

#[cfg(test)]
mod tests;

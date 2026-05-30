use super::request::AgentConversationContextDTO;

const EVIDENCE_SUMMARY_LIMIT: usize = 4;
const WORKSPACE_SECTION_LIMIT: usize = 5;
const LEDGER_EVENT_SECTION_LIMIT: usize = 5;
const LIVE_FEISHU_SECTION_LIMIT: usize = 4;
const ACTIVATED_SKILL_SECTION_LIMIT: usize = 4;

#[derive(Default)]
pub(super) struct AgentSystemPromptBuilder;

impl AgentSystemPromptBuilder {
    pub(super) fn make_prompt(context: &AgentConversationContextDTO) -> String {
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
            title = context.title,
            risk_reason = context.risk_reason,
            action_summary = context.action_summary,
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
        let prompt = AgentSystemPromptBuilder::make_prompt(&AgentConversationContextDTO {
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
            evidence_refs: vec![],
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
            ledger_event_summaries: vec![
                "审计 1｜ActionID act_1｜dry-run 已生成，等待确认".to_string(),
                "审计 2｜ActionID act_2｜用户已查看".to_string(),
                "审计 3｜ActionID act_3｜仍需确认".to_string(),
                "审计 4｜ActionID act_4｜确认已记录".to_string(),
                "审计 5｜ActionID act_5｜AuditEvent 已记录".to_string(),
                "审计 6｜ActionID act_6｜不应进入提示词".to_string(),
            ],
            live_feishu_read_summaries: vec![
                "实时 1".to_string(),
                "实时 2".to_string(),
                "实时 3".to_string(),
                "实时 4".to_string(),
                "实时 5".to_string(),
            ],
            activated_skill_summaries: vec!["feishu.okr｜Feishu OKR｜用途：读取 OKR".to_string()],
        });

        assert!(prompt.contains("工作区级 Agent"));
        assert!(prompt.contains("当前焦点只是本轮请求提供的工作区信号之一"));
        assert!(prompt.contains("不要声称已经读取后端未提供的飞书"));
        assert!(prompt.contains("当前会话历史"));
        assert!(prompt.contains(
            "后端/前端提供的工作区摘要、证据摘要、dry-run 摘要和 review-inbox 安全审计摘要"
        ));
        assert!(prompt.contains("review-inbox 审计摘要是后端/前端提供的安全 OperationLedger 摘要"));
        assert!(prompt.contains("它不是实时 Feishu 读取，不授权写执行，也不证明平台主数据已经改变"));
        assert!(prompt.contains("已激活内置 skill 的领域说明和后端工具说明"));
        assert!(prompt.contains("内置 skill 只是领域和工具说明"));
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
        assert!(prompt.contains("Review-inbox 安全审计摘要"));
        assert!(prompt.contains("1. 审计 1"));
        assert!(prompt.contains("5. 审计 5"));
        assert!(!prompt.contains("审计 6"));
        assert!(prompt.contains("已激活内置 skill"));
        assert!(prompt.contains("feishu.okr｜Feishu OKR"));
        assert!(prompt.contains("实时 Feishu 读取结果"));
        assert!(prompt.contains("后端 tool result 提供的只读实时 Feishu 读取结果"));
        assert!(prompt.contains("实时 Feishu 读取结果只能来自后端 tool result"));
        assert!(prompt.contains("日历忙闲摘要只代表忙碌窗口"));
        assert!(prompt.contains("日程摘要只代表后端返回的受限 agenda 摘要"));
        assert!(prompt.contains("1. 实时 1"));
        assert!(prompt.contains("4. 实时 4"));
        assert!(!prompt.contains("实时 5"));
        assert!(
            prompt.contains("明确区分“前端/既有摘要 / review-inbox 审计摘要”和“后端实时读取结果”")
        );
        for sensitive_term in [
            "raw_payload",
            "raw payload",
            "access token",
            "auth code",
            "credential",
            "secret",
            "full transcript",
            "unredacted",
        ] {
            assert!(
                !prompt.contains(sensitive_term),
                "prompt introduced sensitive/raw term: {sensitive_term}"
            );
        }
    }

    #[test]
    fn prompt_builder_includes_backend_tool_result_summary() {
        let prompt = AgentSystemPromptBuilder::make_prompt(&AgentConversationContextDTO {
            title: "OKR 查询".to_string(),
            risk_reason: "无".to_string(),
            action_summary: "无".to_string(),
            evidence_summaries: vec![],
            evidence_refs: vec![],
            workspace_summary: "摘要".to_string(),
            workspace_signals: vec![],
            pending_action_summaries: vec![],
            ledger_event_summaries: vec![],
            live_feishu_read_summaries: vec![
                "工具 feishu.okr.summarize_my_okr｜实时：我的 OKR 当前有 2 条目标；仅返回安全摘要。"
                    .to_string(),
            ],
            activated_skill_summaries: vec![
                "feishu.okr｜Feishu OKR｜可用后端工具：feishu.okr.summarize_my_okr".to_string(),
            ],
        });

        assert!(prompt.contains("来自后端 tool result"));
        assert!(prompt.contains("feishu.okr.summarize_my_okr"));
        assert!(prompt.contains("我的 OKR 当前有 2 条目标"));
        assert!(!prompt.contains("raw_payload"));
    }

    #[test]
    fn prompt_builder_distinguishes_calendar_free_busy_and_events_summary_boundaries() {
        let prompt = AgentSystemPromptBuilder::make_prompt(&AgentConversationContextDTO {
            title: "日历查询".to_string(),
            risk_reason: "无".to_string(),
            action_summary: "无".to_string(),
            evidence_summaries: vec![],
            evidence_refs: vec![],
            workspace_summary: "摘要".to_string(),
            workspace_signals: vec![],
            pending_action_summaries: vec![],
            ledger_event_summaries: vec![],
            live_feishu_read_summaries: vec![
                "工具 feishu.calendar.summarize_my_free_busy｜实时：未来 7 天读取到 1 个忙碌时段。"
                    .to_string(),
                "工具 feishu.calendar.summarize_my_events｜实时：未来 7 天读取到 1 条日程实例；示例：今天 10:00-11:00，「例会」。"
                    .to_string(),
            ],
            activated_skill_summaries: vec![
                "feishu.calendar｜Feishu Calendar｜可用后端工具：feishu.calendar.summarize_my_free_busy；feishu.calendar.summarize_my_events"
                    .to_string(),
            ],
        });

        assert!(prompt.contains("日历忙闲摘要只代表忙碌窗口"));
        assert!(prompt.contains("日程摘要只代表后端返回的受限 agenda 摘要"));
        assert!(prompt.contains("不代表完整日历记录、描述、会议链接、附件或完整参会人清单"));
        assert!(prompt.contains("feishu.calendar.summarize_my_free_busy"));
        assert!(prompt.contains("feishu.calendar.summarize_my_events"));
    }
}

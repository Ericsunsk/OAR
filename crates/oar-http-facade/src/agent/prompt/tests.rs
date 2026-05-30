use super::context_text::{PROMPT_CONTEXT_TEXT_LIMIT, REDACTED_CONTEXT_SUMMARY};
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
    assert!(prompt
        .contains("后端/前端提供的工作区摘要、证据摘要、dry-run 摘要和 review-inbox 安全审计摘要"));
    assert!(prompt.contains("review-inbox 审计摘要是后端/前端提供的安全 OperationLedger 摘要"));
    assert!(prompt.contains("它不是实时 Feishu 读取，不授权写执行，也不证明平台主数据已经改变"));
    assert!(prompt.contains("已激活内置 skill 的领域说明和后端工具说明"));
    assert!(prompt.contains("内置 skill 只是领域和工具说明"));
    assert!(prompt.contains("必须先有 dry-run"));
    assert!(prompt.contains("ConfirmedAction -> OperationLedger -> PlatformAdapter -> AuditEvent"));
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
    assert!(prompt.contains("明确区分“前端/既有摘要 / review-inbox 审计摘要”和“后端实时读取结果”"));
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
fn prompt_builder_redacts_sensitive_client_context_before_prompting() {
    let prompt = AgentSystemPromptBuilder::make_prompt(&AgentConversationContextDTO {
        title: "KR sk-secret".to_string(),
        risk_reason: "Authorization: Bearer at_live_fake".to_string(),
        action_summary: "raw_payload contains token".to_string(),
        evidence_summaries: vec!["access token leaked".to_string()],
        evidence_refs: vec![],
        workspace_summary: "client_secret should not enter prompt".to_string(),
        workspace_signals: vec!["stdout raw trace".to_string()],
        pending_action_summaries: vec!["credential should not enter prompt".to_string()],
        ledger_event_summaries: vec!["raw_payload sk-secret token leaked".to_string()],
        live_feishu_read_summaries: vec!["refresh_token rt_live_fake".to_string()],
        activated_skill_summaries: vec!["feishu.okr｜Feishu OKR｜用途：读取 OKR".to_string()],
    });

    assert!(prompt.contains(REDACTED_CONTEXT_SUMMARY));
    for forbidden in [
        "sk-secret",
        "Authorization: Bearer",
        "at_live_fake",
        "raw_payload",
        "access token leaked",
        "client_secret",
        "stdout raw trace",
        "credential should not enter prompt",
        "rt_live_fake",
    ] {
        assert!(
            !prompt.contains(forbidden),
            "prompt leaked sensitive client context: {forbidden}"
        );
    }
    assert!(prompt.contains("feishu.okr｜Feishu OKR"));
}

#[test]
fn prompt_builder_compacts_and_truncates_client_context_items() {
    let long_summary = format!(
        "{}{}",
        "长".repeat(PROMPT_CONTEXT_TEXT_LIMIT),
        "尾部不应出现"
    );
    let prompt = AgentSystemPromptBuilder::make_prompt(&AgentConversationContextDTO {
        title: "  KR   风险  ".to_string(),
        risk_reason: "  连续   延期  ".to_string(),
        action_summary: "  更新   进度  ".to_string(),
        evidence_summaries: vec![long_summary],
        evidence_refs: vec![],
        workspace_summary: "  工作区   摘要  ".to_string(),
        workspace_signals: vec![],
        pending_action_summaries: vec![],
        ledger_event_summaries: vec![],
        live_feishu_read_summaries: vec![],
        activated_skill_summaries: vec![],
    });

    assert!(prompt.contains("焦点标题：KR 风险"));
    assert!(prompt.contains("风险信号：连续 延期"));
    assert!(prompt.contains("可用动作和 dry-run：更新 进度"));
    assert!(prompt.contains("工作区 摘要"));
    assert!(prompt.contains("..."));
    assert!(!prompt.contains("尾部不应出现"));
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

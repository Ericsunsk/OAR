use super::assembly::assemble_live_feishu_summaries;
use super::authorization::gate_read_tools_by_scope;
use super::summary::build_task_live_summary;
use super::*;
use crate::agent::request::{AgentConversationContextDTO, AgentEvidenceRefDTO, AgentMessageDTO};
use crate::agent::tools::AgentReadTool;
use oar_core::action::capability::FeishuScope;
use oar_lark_adapter::TaskReadSummary;

#[test]
fn read_tool_scope_gate_requires_real_feishu_oauth_scopes() {
    let mut tools = vec![
        AgentReadTool::OkrSummary,
        AgentReadTool::OkrProgress,
        AgentReadTool::CalendarFreeBusy,
    ];
    let mut degraded = Vec::new();

    gate_read_tools_by_scope(&["okr.content.read".to_string()], &mut tools, &mut degraded);

    assert!(tools.is_empty());
    assert_eq!(degraded.len(), 3);
    assert!(degraded[0].contains("okr:okr.period:readonly"));
    assert!(degraded[0].contains("okr:okr.content:readonly"));
    assert!(degraded[1].contains("okr:okr.period:readonly"));
    assert!(degraded[1].contains("okr:okr.progress:readonly"));
    assert!(degraded[2].contains("calendar:calendar.free_busy:read"));

    let mut tools = vec![AgentReadTool::OkrSummary, AgentReadTool::OkrProgress];
    let mut degraded = Vec::new();

    gate_read_tools_by_scope(
        &[
            FeishuScope::OkrPeriodRead.as_str().to_string(),
            FeishuScope::OkrContentRead.as_str().to_string(),
        ],
        &mut tools,
        &mut degraded,
    );

    assert_eq!(tools, vec![AgentReadTool::OkrSummary]);
    assert_eq!(degraded.len(), 1);
    assert!(degraded[0].contains("feishu.okr.summarize_my_progress"));
    assert!(degraded[0].contains("okr:okr.progress:readonly"));

    let mut tools = vec![
        AgentReadTool::OkrSummary,
        AgentReadTool::OkrProgress,
        AgentReadTool::CalendarFreeBusy,
    ];
    let mut degraded = Vec::new();

    gate_read_tools_by_scope(
        &[
            FeishuScope::OkrPeriodRead.as_str().to_string(),
            FeishuScope::OkrContentRead.as_str().to_string(),
            FeishuScope::OkrProgressRead.as_str().to_string(),
            FeishuScope::CalendarFreeBusyRead.as_str().to_string(),
        ],
        &mut tools,
        &mut degraded,
    );

    assert_eq!(
        tools,
        vec![
            AgentReadTool::OkrSummary,
            AgentReadTool::OkrProgress,
            AgentReadTool::CalendarFreeBusy
        ]
    );
    assert!(degraded.is_empty());
}

#[tokio::test]
async fn live_context_degrades_when_feishu_persistence_is_unavailable() {
    let mut request = live_context_request(
        "请读取实时进展",
        "KR 风险",
        "延期",
        "更新进度",
        vec!["历史摘要"],
        vec![AgentEvidenceRefDTO {
            source_type: "okr".to_string(),
            source_ref: "okr://okr_demo/objectives/obj_demo/krs/kr_demo".to_string(),
            summary: "KR 实时读取".to_string(),
        }],
    );
    let runtime = OarHttpFacadeRuntime::disabled();
    let auth_context = test_auth_context();

    inject_live_feishu_context(&runtime, &auth_context, &mut request).await;

    assert_eq!(request.context.live_feishu_read_summaries.len(), 1);
    assert!(request.context.live_feishu_read_summaries[0].contains("后端未配置 Feishu 授权存储"));
}

#[tokio::test]
async fn live_context_plans_read_only_tool_when_okr_intent_has_no_evidence_refs() {
    let mut request = live_context_request(
        "查我的飞书 OKR 有没有内容",
        "OKR 查询",
        "用户请求实时读取",
        "无",
        vec![],
        vec![],
    );
    let runtime = OarHttpFacadeRuntime::disabled();
    let auth_context = test_auth_context();

    inject_live_feishu_context(&runtime, &auth_context, &mut request).await;

    assert_eq!(request.context.activated_skill_summaries.len(), 1);
    assert!(request.context.activated_skill_summaries[0].contains("feishu.okr"));
    assert_eq!(request.context.live_feishu_read_summaries.len(), 1);
    let summary = &request.context.live_feishu_read_summaries[0];
    assert!(summary.contains("后端未配置 Feishu 授权存储"));
}

#[tokio::test]
async fn live_context_plans_progress_read_tool_when_progress_intent_has_no_evidence_refs() {
    let mut request = live_context_request(
        "我的 OKR 最近更新和风险",
        "OKR 查询",
        "用户请求实时读取",
        "无",
        vec![],
        vec![],
    );
    let runtime = OarHttpFacadeRuntime::disabled();
    let auth_context = test_auth_context();

    inject_live_feishu_context(&runtime, &auth_context, &mut request).await;

    assert_eq!(request.context.activated_skill_summaries.len(), 1);
    assert!(request.context.activated_skill_summaries[0].contains("feishu.okr"));
    assert_eq!(request.context.live_feishu_read_summaries.len(), 1);
    assert!(request.context.live_feishu_read_summaries[0].contains("后端未配置 Feishu 授权存储"));
}

#[tokio::test]
async fn live_context_plans_read_only_tool_when_task_intent_has_no_evidence_refs() {
    let mut request = live_context_request(
        "查下我的飞书任务有几条",
        "任务查询",
        "用户请求实时读取",
        "无",
        vec![],
        vec![],
    );
    let runtime = OarHttpFacadeRuntime::disabled();
    let auth_context = test_auth_context();

    inject_live_feishu_context(&runtime, &auth_context, &mut request).await;

    assert_eq!(request.context.activated_skill_summaries.len(), 1);
    assert!(request.context.activated_skill_summaries[0].contains("feishu.task"));
    assert_eq!(request.context.live_feishu_read_summaries.len(), 1);
    let summary = &request.context.live_feishu_read_summaries[0];
    assert!(summary.contains("后端未配置 Feishu 授权存储"));
}

#[tokio::test]
async fn live_context_plans_read_only_tool_when_calendar_intent_has_no_evidence_refs() {
    let mut request = live_context_request(
        "查下我的飞书日历今天有没有空",
        "日历查询",
        "用户请求实时读取",
        "无",
        vec![],
        vec![],
    );
    let runtime = OarHttpFacadeRuntime::disabled();
    let auth_context = test_auth_context();

    inject_live_feishu_context(&runtime, &auth_context, &mut request).await;

    assert_eq!(request.context.activated_skill_summaries.len(), 1);
    assert!(request.context.activated_skill_summaries[0].contains("feishu.calendar"));
    assert_eq!(request.context.live_feishu_read_summaries.len(), 1);
    let summary = &request.context.live_feishu_read_summaries[0];
    assert!(summary.contains("后端未配置 Feishu 授权存储"));
}

#[tokio::test]
async fn task_live_context_uses_task_refs_before_safe_degrade() {
    let mut request = live_context_request(
        "请读取实时任务",
        "任务风险",
        "待办未闭环",
        "更新任务",
        vec!["历史摘要"],
        vec![AgentEvidenceRefDTO {
            source_type: "task".to_string(),
            source_ref: "feishu://task/task_123".to_string(),
            summary: "任务实时读取".to_string(),
        }],
    );
    let runtime = OarHttpFacadeRuntime::disabled();
    let auth_context = test_auth_context();

    inject_live_feishu_context(&runtime, &auth_context, &mut request).await;

    assert_eq!(request.context.live_feishu_read_summaries.len(), 1);
    assert!(request.context.live_feishu_read_summaries[0].contains("后端未配置 Feishu 授权存储"));
    assert!(!request.context.live_feishu_read_summaries[0].contains("暂不支持实时读取"));
}

#[tokio::test]
async fn live_context_requires_source_type_to_match_task_ref() {
    let refs = vec![AgentEvidenceRefDTO {
        source_type: "doc".to_string(),
        source_ref: "task://sk-secret-ref".to_string(),
        summary: "sk-secret auth code raw transcript".to_string(),
    }];
    let runtime = OarHttpFacadeRuntime::disabled();
    let auth_context = test_auth_context();

    let summaries = assemble_live_feishu_summaries(&runtime, &auth_context, &refs, &[]).await;

    assert_eq!(summaries.len(), 1);
    assert!(summaries[0].contains("source_type 暂不支持实时读取"));
    assert!(!summaries[0].contains("sk-secret"));
    assert!(!summaries[0].contains("auth code"));
    assert!(!summaries[0].contains("raw transcript"));
    assert!(!summaries[0].contains("授权存储"));
}

#[test]
fn task_live_summary_is_sanitized_and_compact() {
    let evidence_ref = AgentEvidenceRefDTO {
        source_type: "task".to_string(),
        source_ref: "task://task_123".to_string(),
        summary: "任务证据".to_string(),
    };
    let summary = build_task_live_summary(
        &evidence_ref,
        &TaskReadSummary {
            source_ref: "task://task_123".to_string(),
            task_id: "task_123".to_string(),
            title: Some(" Ship task read integration ".to_string()),
            status: Some("open".to_string()),
            due: Some(oar_lark_adapter::TaskReadDue {
                timestamp: Some("2026-05-29".to_string()),
                is_all_day: Some(true),
            }),
            owners: vec![oar_lark_adapter::TaskReadOwner {
                owner_id: Some("ou_sensitive".to_string()),
                owner_type: Some("open_id".to_string()),
            }],
            update_time: Some("1717000000".to_string()),
        },
    );

    assert!(summary.contains("任务证据｜实时：任务「Ship task read integration」状态 open"));
    assert!(summary.contains("截止 2026-05-29（全天）"));
    assert!(summary.contains("负责人 1 人"));
    assert!(!summary.contains("ou_sensitive"));
}

fn test_auth_context() -> AuthenticatedContext {
    AuthenticatedContext {
        session_id: "oar_session_test".to_string(),
        tenant_id: "tenant_x".to_string(),
        user_id: "feishu_user_tenant_ou_demo".to_string(),
    }
}

fn live_context_request(
    text: &str,
    title: &str,
    risk_reason: &str,
    action_summary: &str,
    evidence_summaries: Vec<&str>,
    evidence_refs: Vec<AgentEvidenceRefDTO>,
) -> AgentStreamRequest {
    AgentStreamRequest {
        messages: vec![AgentMessageDTO {
            role: "user".to_string(),
            text: text.to_string(),
        }],
        context: AgentConversationContextDTO {
            title: title.to_string(),
            risk_reason: risk_reason.to_string(),
            action_summary: action_summary.to_string(),
            evidence_summaries: evidence_summaries.into_iter().map(str::to_string).collect(),
            evidence_refs,
            workspace_summary: "摘要".to_string(),
            workspace_signals: vec![],
            pending_action_summaries: vec![],
            live_feishu_read_summaries: vec![],
            activated_skill_summaries: vec![],
        },
    }
}

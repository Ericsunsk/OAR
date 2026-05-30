use super::inject_live_feishu_context;
use super::support::{live_context_request, test_auth_context};
use crate::agent::request::AgentEvidenceRefDTO;
use crate::OarHttpFacadeRuntime;

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
async fn live_context_plans_calendar_events_tool_when_agenda_intent_has_no_evidence_refs() {
    let mut request = live_context_request(
        "查下我的飞书日历今天有什么会",
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
    assert!(request.context.activated_skill_summaries[0]
        .contains("feishu.calendar.summarize_my_events"));
    assert_eq!(request.context.live_feishu_read_summaries.len(), 1);
    let summary = &request.context.live_feishu_read_summaries[0];
    assert!(summary.contains("后端未配置 Feishu 授权存储"));
}

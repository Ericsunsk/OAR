use super::support::{live_context_request, test_auth_context};
use super::{assemble_live_feishu_statuses, inject_live_feishu_context};
use crate::agent::live_context::status::LiveFeishuReadState;
use crate::agent::request::AgentEvidenceRefDTO;
use crate::OarHttpFacadeRuntime;

#[tokio::test]
async fn calendar_live_context_uses_calendar_refs_before_safe_degrade() {
    let mut request = live_context_request(
        "请读取实时日程证据",
        "日程风险",
        "会议信息待确认",
        "更新日程判断",
        vec!["历史摘要"],
        vec![AgentEvidenceRefDTO {
            source_type: "lark_calendar".to_string(),
            source_ref: "calendar://cal_1/events/evt_1".to_string(),
            summary: "日程实时读取".to_string(),
        }],
    );
    let runtime = OarHttpFacadeRuntime::disabled();
    let auth_context = test_auth_context();

    inject_live_feishu_context(&runtime, &auth_context, &mut request).await;

    assert_eq!(request.context.live_feishu_read_summaries.len(), 1);
    let summary = &request.context.live_feishu_read_summaries[0];
    assert!(summary.contains("后端未配置 Feishu 授权存储"));
    assert!(!summary.contains("暂不支持实时读取"));
    assert!(!summary.contains("cal_1"));
    assert!(!summary.contains("evt_1"));
    assert_eq!(
        request.context.live_feishu_read_statuses[0].state,
        LiveFeishuReadState::Degraded
    );
}

#[tokio::test]
async fn live_context_degrades_invalid_calendar_refs_without_auth_or_raw_echo() {
    let refs = vec![AgentEvidenceRefDTO {
        source_type: "calendar".to_string(),
        source_ref: "calendar://sk-secret-cal/events/evt%".to_string(),
        summary: "sk-secret auth code raw transcript".to_string(),
    }];
    let runtime = OarHttpFacadeRuntime::disabled();
    let auth_context = test_auth_context();

    let statuses = assemble_live_feishu_statuses(&runtime, &auth_context, &refs, &[]).await;
    let summaries = statuses
        .iter()
        .map(|status| status.summary.as_str())
        .collect::<Vec<_>>();

    assert_eq!(summaries.len(), 1);
    assert_eq!(statuses[0].state, LiveFeishuReadState::Degraded);
    assert!(summaries[0].contains("source_ref 不是可识别的日历引用"));
    assert!(!summaries[0].contains("sk-secret"));
    assert!(!summaries[0].contains("auth code"));
    assert!(!summaries[0].contains("raw transcript"));
    assert!(!summaries[0].contains("授权存储"));
}

#[tokio::test]
async fn live_context_requires_source_type_to_match_calendar_ref() {
    let refs = vec![AgentEvidenceRefDTO {
        source_type: "doc".to_string(),
        source_ref: "calendar://sk-secret-cal/events/sk-secret-event".to_string(),
        summary: "sk-secret auth code raw transcript".to_string(),
    }];
    let runtime = OarHttpFacadeRuntime::disabled();
    let auth_context = test_auth_context();

    let statuses = assemble_live_feishu_statuses(&runtime, &auth_context, &refs, &[]).await;
    let summaries = statuses
        .iter()
        .map(|status| status.summary.as_str())
        .collect::<Vec<_>>();

    assert_eq!(summaries.len(), 1);
    assert_eq!(statuses[0].state, LiveFeishuReadState::Degraded);
    assert!(summaries[0].contains("source_type 暂不支持实时读取"));
    assert!(!summaries[0].contains("sk-secret"));
    assert!(!summaries[0].contains("auth code"));
    assert!(!summaries[0].contains("raw transcript"));
    assert!(!summaries[0].contains("授权存储"));
}

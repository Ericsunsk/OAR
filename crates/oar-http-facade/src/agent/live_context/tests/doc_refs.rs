use super::support::{live_context_request, test_auth_context};
use super::{assemble_live_feishu_statuses, inject_live_feishu_context};
use crate::agent::live_context::status::LiveFeishuReadState;
use crate::agent::request::AgentEvidenceRefDTO;
use crate::OarHttpFacadeRuntime;

#[tokio::test]
async fn doc_live_context_uses_doc_refs_before_safe_degrade() {
    let mut request = live_context_request(
        "请读取实时文档证据",
        "文档风险",
        "文档内容待确认",
        "更新判断",
        vec!["历史摘要"],
        vec![AgentEvidenceRefDTO {
            source_type: "lark_doc".to_string(),
            source_ref: "docx://doxcni6mOy7jLRWbEylaKKabcef".to_string(),
            summary: "文档实时读取".to_string(),
        }],
    );
    let runtime = OarHttpFacadeRuntime::disabled();
    let auth_context = test_auth_context();

    inject_live_feishu_context(&runtime, &auth_context, &mut request).await;

    assert_eq!(request.context.live_feishu_read_summaries.len(), 1);
    let summary = &request.context.live_feishu_read_summaries[0];
    assert!(summary.contains("后端未配置 Feishu 授权存储"));
    assert!(!summary.contains("暂不支持实时读取"));
    assert!(!summary.contains("doxcni6m"));
    assert_eq!(
        request.context.live_feishu_read_statuses[0].state,
        LiveFeishuReadState::Degraded
    );
}

#[tokio::test]
async fn live_context_degrades_invalid_doc_refs_without_auth_or_raw_echo() {
    let refs = vec![AgentEvidenceRefDTO {
        source_type: "doc".to_string(),
        source_ref: "docx://sk-secret-doc?debug=true".to_string(),
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
    assert!(summaries[0].contains("source_ref 不是可识别的文档引用"));
    assert!(!summaries[0].contains("sk-secret"));
    assert!(!summaries[0].contains("auth code"));
    assert!(!summaries[0].contains("raw transcript"));
    assert!(!summaries[0].contains("授权存储"));
}

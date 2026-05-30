use super::support::{live_context_request, test_auth_context};
use super::{assemble_live_feishu_summaries, inject_live_feishu_context};
use crate::agent::request::AgentEvidenceRefDTO;
use crate::OarHttpFacadeRuntime;

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

use super::build_task_live_summary;
use crate::agent::request::AgentEvidenceRefDTO;
use oar_lark_adapter::TaskReadSummary;

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

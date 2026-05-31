use super::{build_doc_live_summary, build_task_live_summary};
use crate::agent::request::AgentEvidenceRefDTO;
use oar_lark_adapter::{DocReadSummary, TaskReadSummary};

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

#[test]
fn doc_live_summary_is_sanitized_and_compact() {
    let evidence_ref = AgentEvidenceRefDTO {
        source_type: "doc".to_string(),
        source_ref: "docx://doxcni6mOy7jLRWbEylaKKabcef".to_string(),
        summary: " 文档证据 ".to_string(),
    };
    let summary = build_doc_live_summary(
        &evidence_ref,
        &DocReadSummary {
            title: Some(" Strategy Launch Notes ".to_string()),
            doc_type: "docx".to_string(),
            revision_id: Some("99".to_string()),
            content_preview: "第一段\n\n包含   多余 空白和重点内容".to_string(),
            content_truncated: true,
            content_char_count: 4096,
        },
    );

    assert!(summary.contains("文档证据｜实时：文档「Strategy Launch Notes」类型 docx"));
    assert!(summary.contains("正文 4096 字，内容已截断"));
    assert!(summary.contains("预览「第一段 包含 多余 空白和重点内容」"));
    assert!(!summary.contains("doxcni6m"));
    assert!(!summary.contains("revision"));
    assert!(!summary.contains("99"));
}

#[test]
fn doc_live_summary_hides_sensitive_preview_markers() {
    let evidence_ref = AgentEvidenceRefDTO {
        source_type: "doc".to_string(),
        source_ref: "docx://doxcni6mOy7jLRWbEylaKKabcef".to_string(),
        summary: "文档证据".to_string(),
    };
    let summary = build_doc_live_summary(
        &evidence_ref,
        &DocReadSummary {
            title: Some("安全处理".to_string()),
            doc_type: "docx".to_string(),
            revision_id: None,
            content_preview: "access_token=sk-secret-token".to_string(),
            content_truncated: false,
            content_char_count: 28,
        },
    );

    assert!(summary.contains("预览已隐藏"));
    assert!(!summary.contains("access_token"));
    assert!(!summary.contains("sk-secret-token"));
    assert!(!summary.contains("doxcni6m"));
}

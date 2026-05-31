use oar_lark_adapter::{OkrReadObjective, OkrReadSnapshot, TaskReadSummary};

use super::text::{compact_text, finalize_summary, truncate_chars};
use crate::agent::live_context::refs::ParsedOkrEvidenceRef;
use crate::agent::request::AgentEvidenceRefDTO;

pub(in crate::agent::live_context) fn build_live_summary(
    evidence_ref: &AgentEvidenceRefDTO,
    parsed: &ParsedOkrEvidenceRef,
    snapshot: &OkrReadSnapshot,
) -> String {
    let label = evidence_label(evidence_ref);
    let Some(okr) = snapshot
        .okrs
        .iter()
        .find(|okr| okr.okr_id.as_deref() == Some(parsed.okr_id.as_str()))
    else {
        return finalize_summary(format!("{label}｜实时：未找到 OKR。"));
    };
    let Some(objective) = okr
        .objectives
        .iter()
        .find(|objective| objective.objective_id.as_deref() == Some(parsed.objective_id.as_str()))
    else {
        return finalize_summary(format!("{label}｜实时：未找到 Objective。"));
    };
    let Some(kr) = objective
        .krs
        .iter()
        .find(|kr| kr.kr_id.as_deref() == Some(parsed.kr_id.as_str()))
    else {
        return finalize_summary(format!("{label}｜实时：未找到 KR。"));
    };

    let kr_content = kr
        .content
        .as_deref()
        .or(objective.content.as_deref())
        .map(compact_text)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "未命名 KR".to_string());
    let progress = kr
        .progress
        .as_deref()
        .or(objective.progress.as_deref())
        .map(compact_text)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "未知".to_string());
    let status = kr
        .status
        .as_deref()
        .or(objective.status.as_deref())
        .map(compact_text)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "未知".to_string());
    let updated_time = latest_update_time(objective);

    finalize_summary(format!(
        "{label}｜实时：KR「{}」进度 {}，状态 {}{}。",
        truncate_chars(&kr_content, 36),
        progress,
        status,
        updated_time
            .map(|time| format!("，更新于 {}", truncate_chars(&compact_text(time), 24)))
            .unwrap_or_default(),
    ))
}

pub(in crate::agent::live_context) fn build_task_live_summary(
    evidence_ref: &AgentEvidenceRefDTO,
    task: &TaskReadSummary,
) -> String {
    let label = evidence_label(evidence_ref);
    let title = task
        .title
        .as_deref()
        .map(compact_text)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "未命名任务".to_string());
    let status = task
        .status
        .as_deref()
        .map(compact_text)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "未知".to_string());
    let due = task
        .due
        .as_ref()
        .and_then(|due| due.timestamp.as_deref())
        .map(compact_text)
        .filter(|value| !value.is_empty())
        .map(|timestamp| {
            if task
                .due
                .as_ref()
                .and_then(|due| due.is_all_day)
                .unwrap_or(false)
            {
                format!("，截止 {}（全天）", truncate_chars(&timestamp, 24))
            } else {
                format!("，截止 {}", truncate_chars(&timestamp, 24))
            }
        })
        .unwrap_or_default();
    let owners = if task.owners.is_empty() {
        String::new()
    } else {
        format!("，负责人 {} 人", task.owners.len())
    };
    let updated_time = task
        .update_time
        .as_deref()
        .map(compact_text)
        .filter(|value| !value.is_empty())
        .map(|time| format!("，更新于 {}", truncate_chars(&time, 24)))
        .unwrap_or_default();

    finalize_summary(format!(
        "{label}｜实时：任务「{}」状态 {}{}{}{}。",
        truncate_chars(&title, 36),
        status,
        due,
        owners,
        updated_time
    ))
}

pub(in crate::agent::live_context) fn degraded_summary(
    _evidence_ref: &AgentEvidenceRefDTO,
    reason: &str,
) -> String {
    finalize_summary(format!("证据｜实时读取降级：{}。", reason))
}

pub(in crate::agent::live_context) fn evidence_unavailable_summary(reason: &str) -> String {
    finalize_summary(format!("未读取到实时 Feishu 证据：{}。", reason))
}

pub(in crate::agent::live_context) fn evidence_label(evidence_ref: &AgentEvidenceRefDTO) -> String {
    let summary = compact_text(&evidence_ref.summary);
    if summary.is_empty() {
        "证据".to_string()
    } else {
        truncate_chars(&summary, 36)
    }
}

fn latest_update_time(objective: &OkrReadObjective) -> Option<&str> {
    objective
        .krs
        .iter()
        .find_map(|kr| kr.last_updated_time.as_deref())
        .or(objective.last_updated_time.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn degraded_summary_does_not_echo_evidence_summary_or_ref() {
        let evidence_ref = AgentEvidenceRefDTO {
            source_type: "task".to_string(),
            source_ref: "task://sk-secret-ref".to_string(),
            summary: "sk-secret auth code raw transcript".to_string(),
        };

        let summary = degraded_summary(&evidence_ref, "任务实时读取暂不可用");

        assert_eq!(summary, "证据｜实时读取降级：任务实时读取暂不可用。");
        assert!(!summary.contains("sk-secret"));
        assert!(!summary.contains("auth code"));
        assert!(!summary.contains("raw transcript"));
    }

    #[test]
    fn evidence_unavailable_summary_uses_global_limit() {
        let summary = evidence_unavailable_summary(&"Feishu 返回空数据".repeat(30));

        assert_eq!(summary.chars().count(), 200);
        assert!(summary.ends_with('…'));
    }
}

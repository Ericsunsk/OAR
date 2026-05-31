use oar_lark_adapter::{
    FeishuCalendarReadError, FeishuOkrReadError, FeishuTaskReadError, OkrReadObjective,
    OkrReadSnapshot, TaskReadSummary,
};

use super::refs::ParsedOkrEvidenceRef;
use crate::agent::request::AgentEvidenceRefDTO;
use crate::agent::tools::AgentReadTool;

const LIVE_SUMMARY_CHAR_LIMIT: usize = 200;

pub(super) fn tool_live_label(tool: AgentReadTool) -> String {
    format!("工具 {}", tool.spec().name)
}

pub(super) fn tool_live_degraded_summary(tool: AgentReadTool, reason: &str) -> String {
    finalize_summary(format!(
        "{}｜实时读取降级：{}。",
        tool_live_label(tool),
        reason
    ))
}

pub(super) fn build_live_summary(
    evidence_ref: &AgentEvidenceRefDTO,
    parsed: &ParsedOkrEvidenceRef,
    snapshot: &OkrReadSnapshot,
) -> String {
    let label = summary_label(evidence_ref);
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

pub(super) fn build_task_live_summary(
    evidence_ref: &AgentEvidenceRefDTO,
    task: &TaskReadSummary,
) -> String {
    let label = summary_label(evidence_ref);
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

pub(super) fn task_read_error_reason(error: FeishuTaskReadError) -> &'static str {
    match error {
        FeishuTaskReadError::InvalidSourceRef => "任务引用无效",
        FeishuTaskReadError::Unauthorized => "授权已失效，需要重新登录",
        FeishuTaskReadError::Forbidden => "授权缺少任务读取权限",
        FeishuTaskReadError::NotFound => "任务不存在或无权访问",
        FeishuTaskReadError::UpstreamClient => "任务读取请求被拒绝",
        FeishuTaskReadError::UpstreamTransient
        | FeishuTaskReadError::Transport
        | FeishuTaskReadError::ApiFailure => "任务实时读取暂不可用",
        FeishuTaskReadError::OversizedResponse | FeishuTaskReadError::InvalidJson => {
            "任务实时读取返回不可用"
        }
    }
}

pub(super) fn calendar_read_error_reason(error: FeishuCalendarReadError) -> &'static str {
    match error {
        FeishuCalendarReadError::InvalidRequest => "日历读取请求无效",
        FeishuCalendarReadError::Unauthorized => "授权已失效，需要重新登录",
        FeishuCalendarReadError::Forbidden => "授权缺少日历读取权限",
        FeishuCalendarReadError::NotFound => "日历目标不存在或无权访问",
        FeishuCalendarReadError::UpstreamClient => "日历读取请求被拒绝",
        FeishuCalendarReadError::UpstreamTransient
        | FeishuCalendarReadError::Transport
        | FeishuCalendarReadError::ApiFailure => "日历实时读取暂不可用",
        FeishuCalendarReadError::OversizedResponse | FeishuCalendarReadError::InvalidJson => {
            "日历实时读取返回不可用"
        }
    }
}

pub(super) fn okr_read_error_reason(error: FeishuOkrReadError) -> &'static str {
    match error {
        FeishuOkrReadError::InvalidRequest => "OKR 读取请求无效",
        FeishuOkrReadError::Unauthorized => "授权已失效，需要重新登录",
        FeishuOkrReadError::Forbidden => "授权缺少 OKR 读取权限",
        FeishuOkrReadError::UpstreamClient => "OKR 读取请求被拒绝",
        FeishuOkrReadError::UpstreamTransient
        | FeishuOkrReadError::Transport
        | FeishuOkrReadError::ApiFailure => "OKR 实时读取暂不可用",
        FeishuOkrReadError::OversizedResponse | FeishuOkrReadError::InvalidJson => {
            "OKR 实时读取返回不可用"
        }
    }
}

pub(super) fn degraded_summary(_evidence_ref: &AgentEvidenceRefDTO, reason: &str) -> String {
    finalize_summary(format!("证据｜实时读取降级：{}。", reason))
}

pub(super) fn summary_label(evidence_ref: &AgentEvidenceRefDTO) -> String {
    let summary = compact_text(&evidence_ref.summary);
    if summary.is_empty() {
        "证据".to_string()
    } else {
        truncate_chars(&summary, 36)
    }
}

pub(super) fn finalize_summary(value: String) -> String {
    truncate_chars(&value, LIVE_SUMMARY_CHAR_LIMIT)
}

fn latest_update_time(objective: &OkrReadObjective) -> Option<&str> {
    objective
        .krs
        .iter()
        .find_map(|kr| kr.last_updated_time.as_deref())
        .or(objective.last_updated_time.as_deref())
}

pub(super) fn compact_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(super) fn truncate_chars(value: &str, limit: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= limit {
        return value.to_string();
    }
    let mut truncated = value
        .chars()
        .take(limit.saturating_sub(1))
        .collect::<String>();
    truncated.push('…');
    truncated
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
    fn tool_live_summaries_use_registered_tool_name() {
        let tool = AgentReadTool::OkrSummary;

        assert_eq!(tool_live_label(tool), format!("工具 {}", tool.spec().name));
        assert_eq!(
            tool_live_degraded_summary(tool, "OKR 实时读取暂不可用"),
            format!(
                "工具 {}｜实时读取降级：OKR 实时读取暂不可用。",
                tool.spec().name
            )
        );
    }
}

use oar_core::action::capability::FeishuScope;

use super::source_registry::LiveEvidenceResolution;
use super::summary::tool_live_degraded_summary;
use crate::agent::tools::AgentReadTool;

pub(super) fn gate_read_demand_by_scope(
    scopes: &[String],
    evidence_resolution: &mut LiveEvidenceResolution<'_>,
    read_tools: &mut Vec<AgentReadTool>,
) -> bool {
    gate_evidence_refs_by_scope(scopes, evidence_resolution);
    gate_read_tools_by_scope(scopes, read_tools, &mut evidence_resolution.degraded);
    !(evidence_resolution.okr_refs.is_empty()
        && evidence_resolution.task_refs.is_empty()
        && read_tools.is_empty())
}

pub(super) fn gate_read_tools_by_scope(
    scopes: &[String],
    read_tools: &mut Vec<AgentReadTool>,
    degraded: &mut Vec<String>,
) {
    read_tools.retain(|tool| {
        let spec = tool.spec();
        let required_scopes = match spec.required_feishu_scope_names() {
            Ok(scopes) => scopes,
            Err(error) => {
                let reason = error.safe_reason();
                degraded.push(tool_live_degraded_summary(*tool, &reason));
                return false;
            }
        };
        let missing = missing_feishu_scope_names(scopes, &required_scopes);
        if missing.is_empty() {
            return true;
        }
        degraded.push(tool_live_degraded_summary(
            *tool,
            &format!("授权缺少 {}", missing.join("、")),
        ));
        false
    });
}

fn gate_evidence_refs_by_scope(scopes: &[String], resolution: &mut LiveEvidenceResolution<'_>) {
    if !resolution.okr_refs.is_empty() && !has_okr_evidence_read_scopes(scopes) {
        resolution.degraded.push(
            "未读取到实时 Feishu OKR 证据：授权缺少 OKR 内容或 progress 读取权限。".to_string(),
        );
        resolution.okr_refs.clear();
    }
    if !resolution.task_refs.is_empty() && !has_task_read_scope(scopes) {
        resolution
            .degraded
            .push("未读取到实时 Feishu 任务证据：授权缺少任务读取权限。".to_string());
        resolution.task_refs.clear();
    }
}

fn has_okr_evidence_read_scopes(scopes: &[String]) -> bool {
    has_feishu_scope(scopes, FeishuScope::OkrContentRead)
        && has_feishu_scope(scopes, FeishuScope::OkrProgressRead)
}

fn has_task_read_scope(scopes: &[String]) -> bool {
    has_feishu_scope(scopes, FeishuScope::TaskRead)
}

fn missing_feishu_scope_names<'a>(
    scopes: &[String],
    required_scopes: &'a [&'static str],
) -> Vec<&'a str> {
    required_scopes
        .iter()
        .filter_map(|required| {
            if scopes.iter().any(|scope| scope.trim() == *required) {
                None
            } else {
                Some(*required)
            }
        })
        .collect()
}

fn has_feishu_scope(scopes: &[String], required: FeishuScope) -> bool {
    let required = required.as_str();
    scopes.iter().any(|scope| scope.trim() == required)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::live_context::source_registry::resolve_evidence_refs;
    use crate::agent::request::AgentEvidenceRefDTO;
    use oar_core::action::capability::OarRequiredScope;

    #[test]
    fn scope_gate_clears_only_sources_missing_their_real_feishu_scope() {
        let refs = vec![
            evidence_ref(
                "okr",
                "okr://okr_demo/objectives/obj_demo/krs/kr_demo",
                "OKR evidence",
            ),
            evidence_ref("task", "task://task_123", "Task evidence"),
        ];
        let mut resolution = resolve_evidence_refs(&refs, 4);

        gate_evidence_refs_by_scope(
            &[
                FeishuScope::OkrContentRead.as_str().to_string(),
                FeishuScope::OkrProgressRead.as_str().to_string(),
            ],
            &mut resolution,
        );

        assert_eq!(resolution.okr_refs.len(), 1);
        assert!(resolution.task_refs.is_empty());
        assert!(resolution
            .degraded
            .iter()
            .any(|summary| summary.contains("授权缺少任务读取权限")));

        let mut resolution = resolve_evidence_refs(&refs, 4);
        gate_evidence_refs_by_scope(
            &[
                FeishuScope::OkrContentRead.as_str().to_string(),
                FeishuScope::TaskRead.as_str().to_string(),
            ],
            &mut resolution,
        );

        assert!(resolution.okr_refs.is_empty());
        assert_eq!(resolution.task_refs.len(), 1);
        assert!(resolution
            .degraded
            .iter()
            .any(|summary| summary.contains("授权缺少 OKR 内容或 progress 读取权限")));
        assert!(!resolution
            .degraded
            .iter()
            .any(|summary| summary.contains("OKR evidence") || summary.contains("okr_demo")));
    }

    #[test]
    fn okr_evidence_read_scope_requires_content_and_progress_feishu_scope_names() {
        assert!(has_okr_evidence_read_scopes(&[
            FeishuScope::OkrContentRead.as_str().to_string(),
            FeishuScope::OkrProgressRead.as_str().to_string(),
        ]));
        assert!(!has_okr_evidence_read_scopes(&[
            FeishuScope::OkrContentRead.as_str().to_string()
        ]));
        assert!(!has_okr_evidence_read_scopes(&[
            FeishuScope::OkrProgressRead.as_str().to_string()
        ]));
        assert!(!has_okr_evidence_read_scopes(&[
            OarRequiredScope::OkrContentRead.as_str().to_string(),
            OarRequiredScope::OkrProgressRead.as_str().to_string(),
        ]));
        assert!(!has_okr_evidence_read_scopes(&[
            "task:task:read".to_string()
        ]));
    }

    #[test]
    fn task_read_scope_accepts_only_feishu_scope_name() {
        assert!(has_task_read_scope(&[FeishuScope::TaskRead
            .as_str()
            .to_string()]));
        assert!(!has_task_read_scope(&["task.read".to_string()]));
        assert!(!has_task_read_scope(&[FeishuScope::OkrProgressRead
            .as_str()
            .to_string()]));
    }

    fn evidence_ref(source_type: &str, source_ref: &str, summary: &str) -> AgentEvidenceRefDTO {
        AgentEvidenceRefDTO {
            source_type: source_type.to_string(),
            source_ref: source_ref.to_string(),
            summary: summary.to_string(),
        }
    }
}

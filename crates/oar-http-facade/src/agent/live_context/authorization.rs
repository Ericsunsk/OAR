use super::source_registry::{gate_evidence_refs_by_scope, LiveEvidenceResolution};
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
        let required_scopes = match spec.required_feishu_scopes() {
            Ok(scopes) => scopes,
            Err(error) => {
                degraded.push(format!(
                    "工具 {}｜实时读取降级：{}。",
                    spec.name,
                    error.safe_reason()
                ));
                return false;
            }
        };
        let missing = required_scopes
            .iter()
            .filter_map(|required| {
                let required = required.as_str();
                if scopes.iter().any(|scope| scope.trim() == required) {
                    None
                } else {
                    Some(required)
                }
            })
            .collect::<Vec<_>>();
        if missing.is_empty() {
            return true;
        }
        degraded.push(format!(
            "工具 {}｜实时读取降级：授权缺少 {}。",
            spec.name,
            missing.join("、")
        ));
        false
    });
}

use super::source_registry::LiveEvidenceResolution;
use super::status::LiveFeishuReadStatus;
use crate::agent::tools::AgentReadTool;

mod evidence;
mod scopes;
mod tools;

pub(super) use self::tools::gate_read_tools_by_scope;

pub(super) fn gate_read_demand_by_scope(
    scopes: &[String],
    evidence_resolution: &mut LiveEvidenceResolution<'_>,
    read_tools: &mut Vec<AgentReadTool>,
    degraded_read_statuses: &mut Vec<LiveFeishuReadStatus>,
) -> bool {
    evidence::gate_evidence_refs_by_scope(scopes, evidence_resolution);
    gate_read_tools_by_scope(scopes, read_tools, degraded_read_statuses);
    !(evidence_resolution.okr_refs.is_empty()
        && evidence_resolution.task_refs.is_empty()
        && evidence_resolution.calendar_refs.is_empty()
        && evidence_resolution.doc_refs.is_empty()
        && evidence_resolution.minutes_refs.is_empty()
        && read_tools.is_empty())
}

#[cfg(test)]
mod tests;

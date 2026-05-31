use std::collections::HashSet;

use crate::agent::tools::AgentReadTool;

use super::super::status::LiveFeishuReadStatus;
use super::scopes::missing_feishu_scope_names;

pub(in crate::agent::live_context) fn gate_read_tools_by_scope(
    scopes: &[String],
    read_tools: &mut Vec<AgentReadTool>,
    degraded: &mut Vec<LiveFeishuReadStatus>,
) {
    dedupe_read_tools(read_tools);
    read_tools.retain(|tool| {
        let spec = tool.spec();
        let required_scopes = match spec.required_feishu_scope_names() {
            Ok(scopes) => scopes,
            Err(error) => {
                let reason = error.safe_reason();
                degraded.push(LiveFeishuReadStatus::degraded_for_tool(*tool, &reason));
                return false;
            }
        };
        let missing = missing_feishu_scope_names(scopes, &required_scopes);
        if missing.is_empty() {
            return true;
        }
        degraded.push(LiveFeishuReadStatus::degraded_for_tool(
            *tool,
            &format!("授权缺少 {}", missing.join("、")),
        ));
        false
    });
}

fn dedupe_read_tools(read_tools: &mut Vec<AgentReadTool>) {
    let mut seen = HashSet::new();
    read_tools.retain(|tool| seen.insert(*tool));
}

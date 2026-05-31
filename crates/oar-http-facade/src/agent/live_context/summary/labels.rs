use super::text::finalize_summary;
use crate::agent::tools::AgentReadTool;

pub(in crate::agent::live_context) fn tool_live_label(tool: AgentReadTool) -> String {
    format!("工具 {}", tool.spec().name)
}

pub(in crate::agent::live_context) fn tool_live_degraded_summary(
    tool: AgentReadTool,
    reason: &str,
) -> String {
    finalize_summary(format!(
        "{}｜实时读取降级：{}。",
        tool_live_label(tool),
        reason
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_live_summaries_use_registered_tool_name() {
        for tool in [
            AgentReadTool::CalendarEvents,
            AgentReadTool::CalendarFreeBusy,
            AgentReadTool::OkrSummary,
            AgentReadTool::OkrProgress,
            AgentReadTool::TaskSummary,
        ] {
            assert_eq!(tool_live_label(tool), format!("工具 {}", tool.spec().name));
            assert_eq!(
                tool_live_degraded_summary(tool, "实时读取暂不可用"),
                format!(
                    "工具 {}｜实时读取降级：实时读取暂不可用。",
                    tool.spec().name
                )
            );
        }
    }
}

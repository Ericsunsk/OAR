use serde::Serialize;

use super::summary::tool_live_degraded_summary;
use crate::agent::tools::AgentReadTool;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LiveFeishuReadState {
    Ready,
    Degraded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::agent) struct LiveFeishuReadStatus {
    pub(in crate::agent) tool: Option<AgentReadTool>,
    pub(in crate::agent) state: LiveFeishuReadState,
    pub(in crate::agent) summary: String,
}

impl LiveFeishuReadStatus {
    pub(in crate::agent) fn ready(summary: String) -> Self {
        Self {
            tool: None,
            state: LiveFeishuReadState::Ready,
            summary,
        }
    }

    pub(in crate::agent) fn ready_for_tool(tool: AgentReadTool, summary: String) -> Self {
        Self {
            tool: Some(tool),
            state: LiveFeishuReadState::Ready,
            summary,
        }
    }

    pub(in crate::agent) fn degraded(summary: String) -> Self {
        Self {
            tool: None,
            state: LiveFeishuReadState::Degraded,
            summary,
        }
    }

    pub(in crate::agent) fn degraded_for_tool(tool: AgentReadTool, reason: &str) -> Self {
        Self {
            tool: Some(tool),
            state: LiveFeishuReadState::Degraded,
            summary: tool_live_degraded_summary(tool, reason),
        }
    }
}

pub(in crate::agent::live_context) fn degraded_statuses(
    summaries: Vec<String>,
) -> Vec<LiveFeishuReadStatus> {
    summaries
        .into_iter()
        .map(LiveFeishuReadStatus::degraded)
        .collect()
}

use crate::agent::skills::{FeishuCalendarReadIntent, FeishuOkrReadIntent};

use super::registry::AgentReadTool;

pub(in crate::agent) fn plan_read_tools_for_activation(
    calendar_intents: &[FeishuCalendarReadIntent],
    okr_intents: &[FeishuOkrReadIntent],
    task_summary_requested: bool,
    minutes_summary_requested: bool,
) -> Vec<AgentReadTool> {
    let mut tools = Vec::new();
    if calendar_intents.contains(&FeishuCalendarReadIntent::FreeBusy) {
        tools.push(AgentReadTool::CalendarFreeBusy);
    }
    if calendar_intents.contains(&FeishuCalendarReadIntent::Events) {
        tools.push(AgentReadTool::CalendarEvents);
    }
    if okr_intents.contains(&FeishuOkrReadIntent::Summary) {
        tools.push(AgentReadTool::OkrSummary);
    }
    if okr_intents.contains(&FeishuOkrReadIntent::Progress) {
        tools.push(AgentReadTool::OkrProgress);
    }
    if task_summary_requested {
        tools.push(AgentReadTool::TaskSummary);
    }
    if minutes_summary_requested {
        tools.push(AgentReadTool::MinutesSummary);
    }

    tools
}

#[cfg(test)]
mod tests;

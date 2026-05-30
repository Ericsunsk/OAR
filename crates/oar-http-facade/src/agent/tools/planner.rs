use crate::agent::request::AgentStreamRequest;
use crate::agent::skills::{AgentSkill, FeishuCalendarReadIntent, FeishuOkrReadIntent};

use super::registry::AgentReadTool;

pub(in crate::agent) fn plan_read_tools(request: &AgentStreamRequest) -> Vec<AgentReadTool> {
    let active_skills = crate::agent::skills::select_skills(request);
    let calendar_intents = crate::agent::skills::select_feishu_calendar_read_intents(request);
    let okr_intents = crate::agent::skills::select_feishu_okr_read_intents(request);
    plan_read_tools_for_selected_intents(&active_skills, &calendar_intents, &okr_intents)
}

fn plan_read_tools_for_selected_intents(
    active_skills: &[AgentSkill],
    calendar_intents: &[FeishuCalendarReadIntent],
    okr_intents: &[FeishuOkrReadIntent],
) -> Vec<AgentReadTool> {
    let mut tools = Vec::new();
    if active_skills.contains(&AgentSkill::Calendar) {
        if calendar_intents.contains(&FeishuCalendarReadIntent::FreeBusy) {
            tools.push(AgentReadTool::CalendarFreeBusy);
        }
        if calendar_intents.contains(&FeishuCalendarReadIntent::Events) {
            tools.push(AgentReadTool::CalendarEvents);
        }
    }
    if active_skills.contains(&AgentSkill::Okr) {
        if okr_intents.contains(&FeishuOkrReadIntent::Summary) {
            tools.push(AgentReadTool::OkrSummary);
        }
        if okr_intents.contains(&FeishuOkrReadIntent::Progress) {
            tools.push(AgentReadTool::OkrProgress);
        }
    }
    if active_skills.contains(&AgentSkill::Task) {
        tools.push(AgentReadTool::TaskSummary);
    }

    tools
}

#[cfg(test)]
mod tests;

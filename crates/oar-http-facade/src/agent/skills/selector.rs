mod calendar;
mod common;
mod okr;
mod task;

#[cfg(test)]
use super::catalog::AgentSkill;
use crate::agent::request::AgentStreamRequest;

pub(in crate::agent) use calendar::FeishuCalendarReadIntent;
pub(in crate::agent) use okr::FeishuOkrReadIntent;

#[cfg(test)]
pub(in crate::agent) fn select_skills(request: &AgentStreamRequest) -> Vec<AgentSkill> {
    let mut skills = Vec::new();
    if !select_feishu_calendar_read_intents(request).is_empty() {
        skills.push(AgentSkill::Calendar);
    }
    let okr_intents = select_feishu_okr_read_intents(request);
    if !okr_intents.is_empty() {
        skills.push(AgentSkill::Okr);
    }
    if select_feishu_task_summary_requested(request) {
        skills.push(AgentSkill::Task);
    }

    skills
}

pub(in crate::agent) fn select_feishu_calendar_read_intents(
    request: &AgentStreamRequest,
) -> Vec<FeishuCalendarReadIntent> {
    calendar::latest_user_feishu_calendar_read_intents(request)
}

pub(in crate::agent) fn select_feishu_okr_read_intents(
    request: &AgentStreamRequest,
) -> Vec<FeishuOkrReadIntent> {
    okr::latest_user_feishu_okr_read_intents(request)
}

pub(in crate::agent) fn select_feishu_task_summary_requested(request: &AgentStreamRequest) -> bool {
    task::latest_user_requests_feishu_task_summary(request)
}

#[cfg(test)]
mod tests;

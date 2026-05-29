mod calendar;
mod common;
mod okr;
mod task;

use super::catalog::AgentSkill;
use crate::agent::request::AgentStreamRequest;

pub(in crate::agent) use okr::FeishuOkrReadIntent;

pub(in crate::agent) fn select_skills(request: &AgentStreamRequest) -> Vec<AgentSkill> {
    let mut skills = Vec::new();
    if calendar::latest_user_requests_feishu_calendar_free_busy(request) {
        skills.push(AgentSkill::Calendar);
    }
    let okr_intents = select_feishu_okr_read_intents(request);
    if !okr_intents.is_empty() {
        skills.push(AgentSkill::Okr);
    }
    if task::latest_user_requests_feishu_task_summary(request) {
        skills.push(AgentSkill::Task);
    }

    skills
}

pub(in crate::agent) fn select_feishu_okr_read_intents(
    request: &AgentStreamRequest,
) -> Vec<FeishuOkrReadIntent> {
    okr::latest_user_feishu_okr_read_intents(request)
}

#[cfg(test)]
mod tests;

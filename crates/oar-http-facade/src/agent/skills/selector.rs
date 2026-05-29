mod calendar;
mod common;
mod okr;
mod task;

use super::catalog::AgentSkill;
use crate::agent::request::AgentStreamRequest;

pub(in crate::agent) fn select_skills(request: &AgentStreamRequest) -> Vec<AgentSkill> {
    let mut skills = Vec::new();
    if calendar::latest_user_requests_feishu_calendar_free_busy(request) {
        skills.push(AgentSkill::FeishuCalendar);
    }
    if okr::latest_user_requests_feishu_okr_summary(request) {
        skills.push(AgentSkill::FeishuOkr);
    }
    if task::latest_user_requests_feishu_task_summary(request) {
        skills.push(AgentSkill::FeishuTask);
    }

    skills
}

#[cfg(test)]
mod tests;

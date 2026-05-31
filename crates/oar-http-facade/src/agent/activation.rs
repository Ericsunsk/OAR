use super::request::AgentStreamRequest;
use super::skills::{
    select_feishu_calendar_read_intents, select_feishu_minutes_summary_requested,
    select_feishu_okr_read_intents, select_feishu_task_summary_requested, AgentSkill,
};
use super::status::AgentActivatedSkillStatus;
use super::tools::{plan_read_tools_for_activation, AgentReadTool};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::agent) struct AgentSkillActivationPlan {
    activated_skills: Vec<AgentSkill>,
    read_tools: Vec<AgentReadTool>,
}

impl AgentSkillActivationPlan {
    pub(in crate::agent) fn activated_skill_summaries(&self) -> Vec<String> {
        self.activated_skills
            .iter()
            .map(|skill| skill.prompt_summary())
            .collect()
    }

    pub(in crate::agent) fn activated_skill_statuses(&self) -> Vec<AgentActivatedSkillStatus> {
        self.activated_skills
            .iter()
            .copied()
            .map(AgentActivatedSkillStatus::from_skill)
            .collect()
    }

    pub(in crate::agent) fn read_tools(&self) -> &[AgentReadTool] {
        &self.read_tools
    }
}

pub(in crate::agent) fn plan_agent_skill_activation(
    request: &AgentStreamRequest,
) -> AgentSkillActivationPlan {
    let calendar_intents = select_feishu_calendar_read_intents(request);
    let okr_intents = select_feishu_okr_read_intents(request);
    let task_summary_requested = select_feishu_task_summary_requested(request);
    let minutes_summary_requested = select_feishu_minutes_summary_requested(request);

    let mut activated_skills = Vec::new();
    if !calendar_intents.is_empty() {
        activated_skills.push(AgentSkill::Calendar);
    }
    if !okr_intents.is_empty() {
        activated_skills.push(AgentSkill::Okr);
    }
    if task_summary_requested {
        activated_skills.push(AgentSkill::Task);
    }
    if minutes_summary_requested {
        activated_skills.push(AgentSkill::Minutes);
    }

    let read_tools = plan_read_tools_for_activation(
        &calendar_intents,
        &okr_intents,
        task_summary_requested,
        minutes_summary_requested,
    );

    AgentSkillActivationPlan {
        activated_skills,
        read_tools,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::request::{AgentConversationContextDTO, AgentMessageDTO};

    #[test]
    fn activation_plan_pairs_okr_skill_summary_and_read_tools() {
        let request = request_with_latest_user_text("查我的 OKR 有几条，以及最近进展");

        let plan = plan_agent_skill_activation(&request);

        assert_eq!(plan.activated_skills, vec![AgentSkill::Okr]);
        assert_eq!(
            plan.read_tools(),
            &[AgentReadTool::OkrSummary, AgentReadTool::OkrProgress]
        );
        assert_eq!(plan.activated_skill_summaries().len(), 1);
        assert!(plan.activated_skill_summaries()[0].contains("feishu.okr"));
        assert_eq!(plan.activated_skill_statuses()[0].id, "feishu.okr");
    }

    #[test]
    fn activation_plan_pairs_calendar_events_skill_summary_and_read_tool() {
        let request = request_with_latest_user_text("查下我的飞书日历今天有什么会");

        let plan = plan_agent_skill_activation(&request);

        assert_eq!(plan.activated_skills, vec![AgentSkill::Calendar]);
        assert_eq!(plan.read_tools(), &[AgentReadTool::CalendarEvents]);
        assert!(plan.activated_skill_summaries()[0].contains("feishu.calendar.summarize_my_events"));
        assert_eq!(plan.activated_skill_statuses()[0].id, "feishu.calendar");
    }

    #[test]
    fn activation_plan_pairs_minutes_skill_summary_and_read_tool() {
        let request = request_with_latest_user_text("查下我的飞书妙记");

        let plan = plan_agent_skill_activation(&request);

        assert_eq!(plan.activated_skills, vec![AgentSkill::Minutes]);
        assert_eq!(plan.read_tools(), &[AgentReadTool::MinutesSummary]);
        assert!(plan.activated_skill_summaries()[0].contains("feishu.minutes.summarize_my_minutes"));
        assert_eq!(plan.activated_skill_statuses()[0].id, "feishu.minutes");
    }

    #[test]
    fn activation_plan_ignores_ambiguous_feishu_count_without_context() {
        let request = request_with_latest_user_text("你看下我飞书目前有几条?");

        let plan = plan_agent_skill_activation(&request);

        assert!(plan.activated_skills.is_empty());
        assert!(plan.read_tools().is_empty());
        assert!(plan.activated_skill_summaries().is_empty());
    }

    fn request_with_latest_user_text(text: &str) -> AgentStreamRequest {
        AgentStreamRequest {
            messages: vec![AgentMessageDTO {
                role: "user".to_string(),
                text: text.to_string(),
            }],
            context: AgentConversationContextDTO {
                title: "未选择风险".to_string(),
                risk_reason: "暂无风险说明。".to_string(),
                action_summary: "暂无建议动作。".to_string(),
                evidence_summaries: vec![],
                evidence_refs: vec![],
                workspace_summary: "暂无工作区摘要。".to_string(),
                workspace_signals: vec![],
                pending_action_summaries: vec![],
                ledger_event_summaries: vec![],
                live_feishu_read_summaries: vec![],
                live_feishu_read_statuses: vec![],
                activated_skill_statuses: vec![],
                activated_skill_summaries: vec![],
            },
        }
    }
}

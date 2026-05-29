#[cfg(test)]
use crate::agent::request::AgentStreamRequest;
use crate::agent::skills::AgentSkill;

use super::registry::AgentReadTool;

#[cfg(test)]
pub(in crate::agent) fn plan_read_tools(request: &AgentStreamRequest) -> Vec<AgentReadTool> {
    let active_skills = crate::agent::skills::select_skills(request);
    plan_read_tools_for_skills(&active_skills)
}

pub(in crate::agent) fn plan_read_tools_for_skills(
    active_skills: &[AgentSkill],
) -> Vec<AgentReadTool> {
    let mut tools = Vec::new();
    if active_skills.contains(&AgentSkill::FeishuCalendar) {
        tools.push(AgentReadTool::FeishuCalendarSummarizeMyFreeBusy);
    }
    if active_skills.contains(&AgentSkill::FeishuOkr) {
        tools.push(AgentReadTool::FeishuOkrSummarizeMyOkr);
    }
    if active_skills.contains(&AgentSkill::FeishuTask) {
        tools.push(AgentReadTool::FeishuTaskSummarizeMyTasks);
    }

    tools
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::request::{AgentConversationContextDTO, AgentMessageDTO};
    use crate::agent::skills::{select_skills, AgentSkill};
    use crate::agent::tools::registry::AgentToolEffect;
    use oar_core::action::capability::{CapabilityActionType, FeishuScope};

    #[test]
    fn planner_requests_my_okr_summary_for_explicit_user_okr_read() {
        let request = request_with_latest_user_text("查下我的飞书 OKR 有没有内容");

        assert_eq!(select_skills(&request), vec![AgentSkill::FeishuOkr]);
        assert_eq!(
            plan_read_tools(&request),
            vec![AgentReadTool::FeishuOkrSummarizeMyOkr]
        );
        let spec = AgentReadTool::FeishuOkrSummarizeMyOkr.spec();
        assert_eq!(spec.name, "feishu.okr.summarize_my_okr");
        assert!(spec.description.contains("只读汇总"));
        assert_eq!(
            spec.required_action_types,
            &[
                CapabilityActionType::OkrPeriodRead,
                CapabilityActionType::OkrContentRead
            ]
        );
        assert_eq!(
            spec.required_feishu_scopes().expect("scopes"),
            vec![FeishuScope::OkrPeriodRead, FeishuScope::OkrContentRead]
        );
        assert_eq!(spec.effect, AgentToolEffect::Read);
    }

    #[test]
    fn planner_requests_my_task_summary_for_explicit_user_task_read() {
        let request = request_with_latest_user_text("查下我的飞书任务有几条");

        assert_eq!(select_skills(&request), vec![AgentSkill::FeishuTask]);
        assert_eq!(
            plan_read_tools(&request),
            vec![AgentReadTool::FeishuTaskSummarizeMyTasks]
        );
        let spec = AgentReadTool::FeishuTaskSummarizeMyTasks.spec();
        assert_eq!(spec.name, "feishu.task.summarize_my_tasks");
        assert!(spec.description.contains("我负责的任务"));
        assert_eq!(
            spec.required_action_types,
            &[CapabilityActionType::TaskRead]
        );
        assert_eq!(
            spec.required_feishu_scopes().expect("scopes"),
            vec![FeishuScope::TaskRead]
        );
        assert_eq!(spec.effect, AgentToolEffect::Read);
    }

    #[test]
    fn planner_requests_my_calendar_free_busy_for_explicit_user_calendar_read() {
        let request = request_with_latest_user_text("查下我的飞书日历今天有没有空");

        assert_eq!(select_skills(&request), vec![AgentSkill::FeishuCalendar]);
        assert_eq!(
            plan_read_tools(&request),
            vec![AgentReadTool::FeishuCalendarSummarizeMyFreeBusy]
        );
        let spec = AgentReadTool::FeishuCalendarSummarizeMyFreeBusy.spec();
        assert_eq!(spec.name, "feishu.calendar.summarize_my_free_busy");
        assert!(spec.description.contains("未来 7 天"));
        assert_eq!(
            spec.required_action_types,
            &[CapabilityActionType::CalendarFreeBusyRead]
        );
        assert_eq!(
            spec.required_feishu_scopes().expect("scopes"),
            vec![FeishuScope::CalendarFreeBusyRead]
        );
        assert_eq!(spec.effect, AgentToolEffect::Read);
    }

    #[test]
    fn planner_ignores_non_okr_or_non_self_scoped_questions() {
        assert!(plan_read_tools(&request_with_latest_user_text("解释这个风险")).is_empty());
        assert!(plan_read_tools(&request_with_latest_user_text("查团队 OKR")).is_empty());
        assert!(plan_read_tools(&request_with_latest_user_text("帮我查团队 OKR")).is_empty());
        assert!(plan_read_tools(&request_with_latest_user_text("查我的目标客户数量")).is_empty());
    }

    #[test]
    fn planner_infers_okr_count_when_latest_feishu_question_continues_okr_topic() {
        let mut request = request_with_latest_user_text("你看下我飞书目前有几条?");
        request.messages.insert(
            0,
            AgentMessageDTO {
                role: "user".to_string(),
                text: "能看到我的 OKR 有几条记录吗".to_string(),
            },
        );

        assert_eq!(select_skills(&request), vec![AgentSkill::FeishuOkr]);
        assert_eq!(
            plan_read_tools(&request),
            vec![AgentReadTool::FeishuOkrSummarizeMyOkr]
        );
    }

    #[test]
    fn planner_does_not_guess_okr_for_ambiguous_feishu_count_without_okr_topic() {
        assert!(
            plan_read_tools(&request_with_latest_user_text("你看下我飞书目前有几条?")).is_empty()
        );
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
                live_feishu_read_summaries: vec![],
                activated_skill_summaries: vec![],
            },
        }
    }
}

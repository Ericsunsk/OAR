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
mod tests {
    use super::*;
    use crate::agent::request::{AgentConversationContextDTO, AgentMessageDTO};
    use crate::agent::skills::{select_skills, AgentSkill};
    use crate::agent::tools::registry::AgentToolEffect;
    use oar_core::action::capability::{CapabilityActionType, FeishuScope};

    #[test]
    fn planner_requests_my_okr_summary_for_explicit_user_okr_read() {
        let request = request_with_latest_user_text("查下我的飞书 OKR 有没有内容");

        assert_eq!(select_skills(&request), vec![AgentSkill::Okr]);
        assert_eq!(plan_read_tools(&request), vec![AgentReadTool::OkrSummary]);
        let spec = AgentReadTool::OkrSummary.spec();
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
    fn planner_requests_my_okr_progress_without_summary_for_progress_intent() {
        let request = request_with_latest_user_text("我的 OKR 最近更新和风险");

        assert_eq!(select_skills(&request), vec![AgentSkill::Okr]);
        assert_eq!(plan_read_tools(&request), vec![AgentReadTool::OkrProgress]);
        let spec = AgentReadTool::OkrProgress.spec();
        assert_eq!(spec.name, "feishu.okr.summarize_my_progress");
        assert!(spec.description.contains("最近更新"));
        assert_eq!(
            spec.required_action_types,
            &[
                CapabilityActionType::OkrPeriodRead,
                CapabilityActionType::OkrContentRead,
                CapabilityActionType::OkrProgressRead
            ]
        );
        assert!(!spec
            .required_action_types
            .contains(&CapabilityActionType::OkrProgressCreate));
        assert!(!spec
            .required_action_types
            .contains(&CapabilityActionType::OkrProgressUpdate));
        let scopes = spec.required_feishu_scopes().expect("scopes");
        assert_eq!(
            scopes,
            vec![
                FeishuScope::OkrPeriodRead,
                FeishuScope::OkrContentRead,
                FeishuScope::OkrProgressRead
            ]
        );
        assert!(!scopes.contains(&FeishuScope::OkrProgressWrite));
        assert_eq!(spec.effect, AgentToolEffect::Read);
    }

    #[test]
    fn planner_requests_only_progress_for_target_progress_phrasing() {
        for text in ["看我的 OKR 目标进展", "show my OKR objective progress"] {
            let request = request_with_latest_user_text(text);

            assert_eq!(select_skills(&request), vec![AgentSkill::Okr], "{text}");
            assert_eq!(
                plan_read_tools(&request),
                vec![AgentReadTool::OkrProgress],
                "{text}"
            );
        }
    }

    #[test]
    fn planner_requests_both_okr_tools_for_count_and_progress_intent() {
        let request = request_with_latest_user_text("查我的 OKR 有几条，以及最近进展");

        assert_eq!(select_skills(&request), vec![AgentSkill::Okr]);
        assert_eq!(
            plan_read_tools(&request),
            vec![AgentReadTool::OkrSummary, AgentReadTool::OkrProgress]
        );
    }

    #[test]
    fn planner_requests_read_tools_when_user_explicitly_retries_tool_ids() {
        assert_eq!(
            plan_read_tools(&request_with_latest_user_text(
                "请重试 `feishu.okr.summarize_my_okr`"
            )),
            vec![AgentReadTool::OkrSummary]
        );
        assert_eq!(
            plan_read_tools(&request_with_latest_user_text(
                "retry feishu.okr.summarize_my_progress"
            )),
            vec![AgentReadTool::OkrProgress]
        );
        assert_eq!(
            plan_read_tools(&request_with_latest_user_text(
                "重新读取 feishu.task.summarize_my_tasks"
            )),
            vec![AgentReadTool::TaskSummary]
        );
        assert_eq!(
            plan_read_tools(&request_with_latest_user_text(
                "run feishu.calendar.summarize_my_free_busy"
            )),
            vec![AgentReadTool::CalendarFreeBusy]
        );
        assert_eq!(
            plan_read_tools(&request_with_latest_user_text(
                "run feishu.calendar.summarize_my_events"
            )),
            vec![AgentReadTool::CalendarEvents]
        );
    }

    #[test]
    fn planner_requests_my_task_summary_for_explicit_user_task_read() {
        let request = request_with_latest_user_text("查下我的飞书任务有几条");

        assert_eq!(select_skills(&request), vec![AgentSkill::Task]);
        assert_eq!(plan_read_tools(&request), vec![AgentReadTool::TaskSummary]);
        let spec = AgentReadTool::TaskSummary.spec();
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

        assert_eq!(select_skills(&request), vec![AgentSkill::Calendar]);
        assert_eq!(
            plan_read_tools(&request),
            vec![AgentReadTool::CalendarFreeBusy]
        );
        let spec = AgentReadTool::CalendarFreeBusy.spec();
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
    fn planner_requests_my_calendar_events_for_agenda_read() {
        let request = request_with_latest_user_text("查下我的飞书日历今天有什么会");

        assert_eq!(select_skills(&request), vec![AgentSkill::Calendar]);
        assert_eq!(
            plan_read_tools(&request),
            vec![AgentReadTool::CalendarEvents]
        );
        let spec = AgentReadTool::CalendarEvents.spec();
        assert_eq!(spec.name, "feishu.calendar.summarize_my_events");
        assert!(spec.description.contains("受限摘要"));
        assert_eq!(
            spec.required_action_types,
            &[
                CapabilityActionType::CalendarRead,
                CapabilityActionType::CalendarEventRead
            ]
        );
        assert_eq!(
            spec.required_feishu_scopes().expect("scopes"),
            vec![FeishuScope::CalendarRead, FeishuScope::CalendarEventRead]
        );
        assert_eq!(spec.effect, AgentToolEffect::Read);
    }

    #[test]
    fn planner_ignores_non_okr_or_non_self_scoped_questions() {
        assert!(plan_read_tools(&request_with_latest_user_text("解释这个风险")).is_empty());
        assert!(plan_read_tools(&request_with_latest_user_text("查团队 OKR")).is_empty());
        assert!(plan_read_tools(&request_with_latest_user_text("帮我查团队 OKR")).is_empty());
        assert!(plan_read_tools(&request_with_latest_user_text("查我的目标客户数量")).is_empty());
        assert!(
            plan_read_tools(&request_with_latest_user_text("帮我更新我的 OKR 进度")).is_empty()
        );
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

        assert_eq!(select_skills(&request), vec![AgentSkill::Okr]);
        assert_eq!(plan_read_tools(&request), vec![AgentReadTool::OkrSummary]);
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

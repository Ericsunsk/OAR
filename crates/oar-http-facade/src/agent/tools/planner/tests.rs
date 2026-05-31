use super::*;
use crate::agent::skills::{FeishuCalendarReadIntent, FeishuOkrReadIntent};
use crate::agent::tools::registry::AgentToolEffect;
use oar_core::action::capability::{CapabilityActionType, FeishuScope};

#[test]
fn planner_maps_okr_summary_intent_to_summary_tool() {
    assert_eq!(
        planned_read_tools(&[], &[FeishuOkrReadIntent::Summary], false, false),
        vec![AgentReadTool::OkrSummary]
    );
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
fn planner_maps_okr_progress_intent_to_progress_tool() {
    assert_eq!(
        planned_read_tools(&[], &[FeishuOkrReadIntent::Progress], false, false),
        vec![AgentReadTool::OkrProgress]
    );
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
fn planner_maps_task_summary_request_to_task_tool() {
    assert_eq!(
        planned_read_tools(&[], &[], true, false),
        vec![AgentReadTool::TaskSummary]
    );
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
fn planner_maps_calendar_free_busy_intent_to_free_busy_tool() {
    assert_eq!(
        planned_read_tools(&[FeishuCalendarReadIntent::FreeBusy], &[], false, false),
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
fn planner_maps_calendar_events_intent_to_events_tool() {
    assert_eq!(
        planned_read_tools(&[FeishuCalendarReadIntent::Events], &[], false, false),
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
fn planner_maps_minutes_summary_request_to_minutes_tool() {
    assert_eq!(
        planned_read_tools(&[], &[], false, true),
        vec![AgentReadTool::MinutesSummary]
    );
    let spec = AgentReadTool::MinutesSummary.spec();
    assert_eq!(spec.name, "feishu.minutes.summarize_my_minutes");
    assert!(spec.description.contains("meeting notes"));
    assert_eq!(
        spec.required_action_types,
        &[CapabilityActionType::MinutesSearchRead]
    );
    assert_eq!(
        spec.required_feishu_scopes().expect("scopes"),
        vec![FeishuScope::MinutesSearchRead]
    );
    assert_eq!(spec.effect, AgentToolEffect::Read);
}

#[test]
fn planner_preserves_calendar_okr_task_tool_order() {
    assert_eq!(
        planned_read_tools(
            &[
                FeishuCalendarReadIntent::FreeBusy,
                FeishuCalendarReadIntent::Events,
            ],
            &[FeishuOkrReadIntent::Summary, FeishuOkrReadIntent::Progress,],
            true,
            true,
        ),
        vec![
            AgentReadTool::CalendarFreeBusy,
            AgentReadTool::CalendarEvents,
            AgentReadTool::OkrSummary,
            AgentReadTool::OkrProgress,
            AgentReadTool::TaskSummary,
            AgentReadTool::MinutesSummary,
        ]
    );
}

#[test]
fn planner_returns_no_tools_without_selected_intents() {
    assert!(planned_read_tools(&[], &[], false, false).is_empty());
}

fn planned_read_tools(
    calendar_intents: &[FeishuCalendarReadIntent],
    okr_intents: &[FeishuOkrReadIntent],
    task_summary_requested: bool,
    minutes_summary_requested: bool,
) -> Vec<AgentReadTool> {
    plan_read_tools_for_activation(
        calendar_intents,
        okr_intents,
        task_summary_requested,
        minutes_summary_requested,
    )
}

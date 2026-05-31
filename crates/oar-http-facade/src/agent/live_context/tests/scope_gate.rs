use super::gate_read_tools_by_scope;
use crate::agent::live_context::status::LiveFeishuReadState;
use crate::agent::tools::AgentReadTool;
use oar_core::action::capability::FeishuScope;

#[test]
fn read_tool_scope_gate_requires_real_feishu_oauth_scopes() {
    let mut tools = vec![
        AgentReadTool::OkrSummary,
        AgentReadTool::OkrProgress,
        AgentReadTool::CalendarEvents,
        AgentReadTool::CalendarFreeBusy,
        AgentReadTool::MinutesSummary,
    ];
    let mut degraded = Vec::new();

    gate_read_tools_by_scope(&["okr.content.read".to_string()], &mut tools, &mut degraded);

    assert!(tools.is_empty());
    assert_eq!(degraded.len(), 5);
    assert_eq!(degraded[0].tool, Some(AgentReadTool::OkrSummary));
    assert_eq!(degraded[0].state, LiveFeishuReadState::Degraded);
    assert!(degraded[0].summary.contains("okr:okr.period:readonly"));
    assert!(degraded[0].summary.contains("okr:okr.content:readonly"));
    assert_eq!(degraded[1].tool, Some(AgentReadTool::OkrProgress));
    assert!(degraded[1].summary.contains("okr:okr.period:readonly"));
    assert!(degraded[1].summary.contains("okr:okr.progress:readonly"));
    assert_eq!(degraded[2].tool, Some(AgentReadTool::CalendarEvents));
    assert!(degraded[2].summary.contains("calendar:calendar:read"));
    assert!(degraded[2].summary.contains("calendar:calendar.event:read"));
    assert_eq!(degraded[3].tool, Some(AgentReadTool::CalendarFreeBusy));
    assert!(degraded[3]
        .summary
        .contains("calendar:calendar.free_busy:read"));
    assert_eq!(degraded[4].tool, Some(AgentReadTool::MinutesSummary));
    assert!(degraded[4].summary.contains("minutes:minutes.search:read"));

    let mut tools = vec![AgentReadTool::OkrSummary, AgentReadTool::OkrProgress];
    let mut degraded = Vec::new();

    gate_read_tools_by_scope(
        &[
            FeishuScope::OkrPeriodRead.as_str().to_string(),
            FeishuScope::OkrContentRead.as_str().to_string(),
        ],
        &mut tools,
        &mut degraded,
    );

    assert_eq!(tools, vec![AgentReadTool::OkrSummary]);
    assert_eq!(degraded.len(), 1);
    assert_eq!(degraded[0].tool, Some(AgentReadTool::OkrProgress));
    assert!(degraded[0]
        .summary
        .contains("feishu.okr.summarize_my_progress"));
    assert!(degraded[0].summary.contains("okr:okr.progress:readonly"));

    let mut tools = vec![
        AgentReadTool::OkrSummary,
        AgentReadTool::OkrProgress,
        AgentReadTool::CalendarEvents,
        AgentReadTool::CalendarFreeBusy,
        AgentReadTool::MinutesSummary,
    ];
    let mut degraded = Vec::new();

    gate_read_tools_by_scope(
        &[
            FeishuScope::OkrPeriodRead.as_str().to_string(),
            FeishuScope::OkrContentRead.as_str().to_string(),
            FeishuScope::OkrProgressRead.as_str().to_string(),
            FeishuScope::CalendarRead.as_str().to_string(),
            FeishuScope::CalendarEventRead.as_str().to_string(),
            FeishuScope::CalendarFreeBusyRead.as_str().to_string(),
            FeishuScope::MinutesSearchRead.as_str().to_string(),
        ],
        &mut tools,
        &mut degraded,
    );

    assert_eq!(
        tools,
        vec![
            AgentReadTool::OkrSummary,
            AgentReadTool::OkrProgress,
            AgentReadTool::CalendarEvents,
            AgentReadTool::CalendarFreeBusy,
            AgentReadTool::MinutesSummary
        ]
    );
    assert!(degraded.is_empty());
}

#[test]
fn read_tool_scope_gate_deduplicates_tools_before_scope_checks() {
    let mut tools = vec![
        AgentReadTool::OkrSummary,
        AgentReadTool::OkrSummary,
        AgentReadTool::OkrProgress,
        AgentReadTool::OkrProgress,
    ];
    let mut degraded = Vec::new();

    gate_read_tools_by_scope(&[], &mut tools, &mut degraded);

    assert!(tools.is_empty());
    assert_eq!(degraded.len(), 2);
    assert_eq!(
        degraded
            .iter()
            .filter(|status| status.summary.contains("feishu.okr.summarize_my_okr"))
            .count(),
        1
    );
    assert_eq!(
        degraded
            .iter()
            .filter(|status| status.summary.contains("feishu.okr.summarize_my_progress"))
            .count(),
        1
    );

    let mut tools = vec![AgentReadTool::OkrSummary, AgentReadTool::OkrSummary];
    let mut degraded = Vec::new();

    gate_read_tools_by_scope(
        &[
            FeishuScope::OkrPeriodRead.as_str().to_string(),
            FeishuScope::OkrContentRead.as_str().to_string(),
        ],
        &mut tools,
        &mut degraded,
    );

    assert_eq!(tools, vec![AgentReadTool::OkrSummary]);
    assert!(degraded.is_empty());
}

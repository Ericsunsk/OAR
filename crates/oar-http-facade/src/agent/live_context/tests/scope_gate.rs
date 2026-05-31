use super::gate_read_tools_by_scope;
use crate::agent::tools::AgentReadTool;
use oar_core::action::capability::FeishuScope;

#[test]
fn read_tool_scope_gate_requires_real_feishu_oauth_scopes() {
    let mut tools = vec![
        AgentReadTool::OkrSummary,
        AgentReadTool::OkrProgress,
        AgentReadTool::CalendarEvents,
        AgentReadTool::CalendarFreeBusy,
    ];
    let mut degraded = Vec::new();

    gate_read_tools_by_scope(&["okr.content.read".to_string()], &mut tools, &mut degraded);

    assert!(tools.is_empty());
    assert_eq!(degraded.len(), 4);
    assert!(degraded[0].contains("okr:okr.period:readonly"));
    assert!(degraded[0].contains("okr:okr.content:readonly"));
    assert!(degraded[1].contains("okr:okr.period:readonly"));
    assert!(degraded[1].contains("okr:okr.progress:readonly"));
    assert!(degraded[2].contains("calendar:calendar:read"));
    assert!(degraded[2].contains("calendar:calendar.event:read"));
    assert!(degraded[3].contains("calendar:calendar.free_busy:read"));

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
    assert!(degraded[0].contains("feishu.okr.summarize_my_progress"));
    assert!(degraded[0].contains("okr:okr.progress:readonly"));

    let mut tools = vec![
        AgentReadTool::OkrSummary,
        AgentReadTool::OkrProgress,
        AgentReadTool::CalendarEvents,
        AgentReadTool::CalendarFreeBusy,
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
            AgentReadTool::CalendarFreeBusy
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
            .filter(|summary| summary.contains("feishu.okr.summarize_my_okr"))
            .count(),
        1
    );
    assert_eq!(
        degraded
            .iter()
            .filter(|summary| summary.contains("feishu.okr.summarize_my_progress"))
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

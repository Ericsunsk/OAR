use super::gate_read_tools_by_scope;
use crate::agent::tools::AgentReadTool;
use oar_core::action::capability::FeishuScope;

#[test]
fn read_tool_scope_gate_requires_real_feishu_oauth_scopes() {
    let mut tools = vec![
        AgentReadTool::OkrSummary,
        AgentReadTool::OkrProgress,
        AgentReadTool::CalendarFreeBusy,
    ];
    let mut degraded = Vec::new();

    gate_read_tools_by_scope(&["okr.content.read".to_string()], &mut tools, &mut degraded);

    assert!(tools.is_empty());
    assert_eq!(degraded.len(), 3);
    assert!(degraded[0].contains("okr:okr.period:readonly"));
    assert!(degraded[0].contains("okr:okr.content:readonly"));
    assert!(degraded[1].contains("okr:okr.period:readonly"));
    assert!(degraded[1].contains("okr:okr.progress:readonly"));
    assert!(degraded[2].contains("calendar:calendar.free_busy:read"));

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
        AgentReadTool::CalendarFreeBusy,
    ];
    let mut degraded = Vec::new();

    gate_read_tools_by_scope(
        &[
            FeishuScope::OkrPeriodRead.as_str().to_string(),
            FeishuScope::OkrContentRead.as_str().to_string(),
            FeishuScope::OkrProgressRead.as_str().to_string(),
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
            AgentReadTool::CalendarFreeBusy
        ]
    );
    assert!(degraded.is_empty());
}

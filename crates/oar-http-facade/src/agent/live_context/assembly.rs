mod calendar;
mod okr;
mod task;

use calendar::append_calendar_summary;
use okr::append_okr_summaries;
use task::append_task_summaries;

use super::authorization::gate_read_demand_by_scope;
use super::session::LiveFeishuReadSession;
use super::source_registry::resolve_evidence_refs;
use super::status::{degraded_statuses, LiveFeishuReadStatus};
use super::LIVE_EVIDENCE_REF_LIMIT;
use crate::agent::request::AgentEvidenceRefDTO;
use crate::agent::tools::AgentReadTool;
use crate::{AuthenticatedContext, OarHttpFacadeRuntime};

pub(super) async fn assemble_live_feishu_statuses(
    runtime: &OarHttpFacadeRuntime,
    auth_context: &AuthenticatedContext,
    evidence_refs: &[AgentEvidenceRefDTO],
    planned_read_tools: &[AgentReadTool],
) -> Vec<LiveFeishuReadStatus> {
    if evidence_refs.is_empty() && planned_read_tools.is_empty() {
        return vec![];
    }

    let mut evidence_resolution = resolve_evidence_refs(evidence_refs, LIVE_EVIDENCE_REF_LIMIT);
    let mut read_tools = planned_read_tools.to_vec();

    if evidence_resolution.okr_refs.is_empty()
        && evidence_resolution.task_refs.is_empty()
        && read_tools.is_empty()
    {
        return degraded_statuses(evidence_resolution.degraded);
    }

    let mut gated_statuses = Vec::new();
    let session_result = LiveFeishuReadSession::open(runtime, auth_context, |scopes| {
        gate_read_demand_by_scope(
            scopes,
            &mut evidence_resolution,
            &mut read_tools,
            &mut gated_statuses,
        )
    })
    .await;
    let session = match session_result {
        Ok(session) => session,
        Err(error) => {
            error.push_degraded(&mut evidence_resolution.degraded);
            gated_statuses.extend(degraded_statuses(evidence_resolution.degraded));
            return gated_statuses;
        }
    };

    let planned_reads = PlannedLiveReads::from_tools(&read_tools);
    let lark_open_id_for_tool_reads = if planned_reads.needs_lark_open_id() {
        Some(session.resolve_lark_open_id(auth_context).await)
    } else {
        None
    };

    let mut live_statuses = gated_statuses;
    append_okr_summaries(
        &mut live_statuses,
        &session,
        &mut evidence_resolution,
        planned_reads,
        &lark_open_id_for_tool_reads,
    )
    .await;
    append_task_summaries(
        &mut live_statuses,
        &session,
        &mut evidence_resolution,
        planned_reads,
    )
    .await;
    append_calendar_summary(
        &mut live_statuses,
        &session,
        planned_reads,
        &lark_open_id_for_tool_reads,
    )
    .await;

    live_statuses.extend(degraded_statuses(evidence_resolution.degraded));
    live_statuses
}

#[derive(Clone, Copy)]
pub(super) struct PlannedLiveReads {
    pub(super) okr_summary: bool,
    pub(super) okr_progress: bool,
    pub(super) task_summary: bool,
    pub(super) calendar_events: bool,
    pub(super) calendar_free_busy: bool,
}

impl PlannedLiveReads {
    fn from_tools(read_tools: &[AgentReadTool]) -> Self {
        Self {
            okr_summary: read_tools.contains(&AgentReadTool::OkrSummary),
            okr_progress: read_tools.contains(&AgentReadTool::OkrProgress),
            task_summary: read_tools.contains(&AgentReadTool::TaskSummary),
            calendar_events: read_tools.contains(&AgentReadTool::CalendarEvents),
            calendar_free_busy: read_tools.contains(&AgentReadTool::CalendarFreeBusy),
        }
    }

    fn needs_lark_open_id(self) -> bool {
        self.okr_summary || self.okr_progress || self.calendar_free_busy
    }
}

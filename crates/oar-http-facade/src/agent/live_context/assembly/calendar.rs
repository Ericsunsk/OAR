use super::super::calendar_summary::{
    build_calendar_event_live_summary, read_my_calendar_events_summary,
    read_my_calendar_free_busy_summary,
};
use super::super::session::LiveFeishuReadSession;
use super::super::source_registry::LiveEvidenceResolution;
use super::super::status::LiveFeishuReadStatus;
use super::super::summary::{calendar_read_error_reason, degraded_summary};
use super::PlannedLiveReads;
use crate::agent::tools::AgentReadTool;
use oar_lark_adapter::{
    AsyncFeishuCalendarRead, CalendarEventReadRequest, FeishuCalendarReadClient,
    ReqwestAsyncHttpClient,
};

pub(super) async fn append_calendar_summary(
    live_statuses: &mut Vec<LiveFeishuReadStatus>,
    session: &LiveFeishuReadSession,
    evidence_resolution: &mut LiveEvidenceResolution<'_>,
    planned_reads: PlannedLiveReads,
    lark_open_id_for_tool_reads: &Option<Result<String, &'static str>>,
) {
    if evidence_resolution.calendar_refs.is_empty()
        && !planned_reads.calendar_free_busy
        && !planned_reads.calendar_events
    {
        return;
    }

    let mut calendar_client = session.calendar_client();

    if planned_reads.calendar_free_busy {
        match lark_open_id_for_tool_reads {
            Some(Ok(lark_open_id)) => {
                match read_my_calendar_free_busy_summary(
                    &mut calendar_client,
                    session.access_token(),
                    lark_open_id,
                    session.now(),
                )
                .await
                {
                    Ok(summary) => live_statuses.push(LiveFeishuReadStatus::ready_for_tool(
                        AgentReadTool::CalendarFreeBusy,
                        summary,
                    )),
                    Err(error) => live_statuses.push(LiveFeishuReadStatus::degraded_for_tool(
                        AgentReadTool::CalendarFreeBusy,
                        calendar_read_error_reason(error),
                    )),
                }
            }
            Some(Err(reason)) => live_statuses.push(LiveFeishuReadStatus::degraded_for_tool(
                AgentReadTool::CalendarFreeBusy,
                reason,
            )),
            None => live_statuses.push(LiveFeishuReadStatus::degraded_for_tool(
                AgentReadTool::CalendarFreeBusy,
                "用户身份未解析",
            )),
        }
    }

    if planned_reads.calendar_events {
        append_calendar_events_summary(live_statuses, session, &mut calendar_client).await;
    }

    for (evidence_ref, parsed) in std::mem::take(&mut evidence_resolution.calendar_refs) {
        match calendar_client
            .get_event_summary(CalendarEventReadRequest {
                user_access_token: session.access_token(),
                source_ref: parsed.source_ref(),
            })
            .await
        {
            Ok(event) => {
                live_statuses.push(LiveFeishuReadStatus::ready(
                    build_calendar_event_live_summary(evidence_ref, &event),
                ));
            }
            Err(error) => {
                live_statuses.push(LiveFeishuReadStatus::degraded(degraded_summary(
                    evidence_ref,
                    calendar_read_error_reason(error),
                )));
            }
        }
    }
}

async fn append_calendar_events_summary(
    live_statuses: &mut Vec<LiveFeishuReadStatus>,
    session: &LiveFeishuReadSession,
    calendar_client: &mut FeishuCalendarReadClient<ReqwestAsyncHttpClient>,
) {
    match read_my_calendar_events_summary(calendar_client, session.access_token(), session.now())
        .await
    {
        Ok(summary) => live_statuses.push(LiveFeishuReadStatus::ready_for_tool(
            AgentReadTool::CalendarEvents,
            summary,
        )),
        Err(error) => live_statuses.push(LiveFeishuReadStatus::degraded_for_tool(
            AgentReadTool::CalendarEvents,
            calendar_read_error_reason(error),
        )),
    }
}

use super::super::calendar_summary::{
    read_my_calendar_events_summary, read_my_calendar_free_busy_summary,
};
use super::super::session::LiveFeishuReadSession;
use super::super::status::LiveFeishuReadStatus;
use super::super::summary::calendar_read_error_reason;
use super::PlannedLiveReads;
use crate::agent::tools::AgentReadTool;

pub(super) async fn append_calendar_summary(
    live_statuses: &mut Vec<LiveFeishuReadStatus>,
    session: &LiveFeishuReadSession,
    planned_reads: PlannedLiveReads,
    lark_open_id_for_tool_reads: &Option<Result<String, &'static str>>,
) {
    if !planned_reads.calendar_free_busy {
        if planned_reads.calendar_events {
            append_calendar_events_summary(live_statuses, session).await;
        }
        return;
    }

    let mut calendar_client = session.calendar_client();
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

    if planned_reads.calendar_events {
        append_calendar_events_summary(live_statuses, session).await;
    }
}

async fn append_calendar_events_summary(
    live_statuses: &mut Vec<LiveFeishuReadStatus>,
    session: &LiveFeishuReadSession,
) {
    let mut calendar_client = session.calendar_client();
    match read_my_calendar_events_summary(
        &mut calendar_client,
        session.access_token(),
        session.now(),
    )
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

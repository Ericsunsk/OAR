use super::super::calendar_summary::{
    read_my_calendar_events_summary, read_my_calendar_free_busy_summary,
};
use super::super::session::LiveFeishuReadSession;
use super::super::summary::{calendar_read_error_reason, tool_live_degraded_summary};
use super::PlannedLiveReads;
use crate::agent::tools::AgentReadTool;

pub(super) async fn append_calendar_summary(
    live_summaries: &mut Vec<String>,
    session: &LiveFeishuReadSession,
    planned_reads: PlannedLiveReads,
    lark_open_id_for_tool_reads: &Option<Result<String, &'static str>>,
) {
    if !planned_reads.calendar_free_busy {
        if planned_reads.calendar_events {
            append_calendar_events_summary(live_summaries, session).await;
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
                Ok(summary) => live_summaries.push(summary),
                Err(error) => live_summaries.push(tool_live_degraded_summary(
                    AgentReadTool::CalendarFreeBusy,
                    calendar_read_error_reason(error),
                )),
            }
        }
        Some(Err(reason)) => live_summaries.push(tool_live_degraded_summary(
            AgentReadTool::CalendarFreeBusy,
            reason,
        )),
        None => live_summaries.push(tool_live_degraded_summary(
            AgentReadTool::CalendarFreeBusy,
            "用户身份未解析",
        )),
    }

    if planned_reads.calendar_events {
        append_calendar_events_summary(live_summaries, session).await;
    }
}

async fn append_calendar_events_summary(
    live_summaries: &mut Vec<String>,
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
        Ok(summary) => live_summaries.push(summary),
        Err(error) => live_summaries.push(tool_live_degraded_summary(
            AgentReadTool::CalendarEvents,
            calendar_read_error_reason(error),
        )),
    }
}

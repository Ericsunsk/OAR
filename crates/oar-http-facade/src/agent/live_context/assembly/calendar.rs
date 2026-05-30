use super::super::calendar_summary::read_my_calendar_free_busy_summary;
use super::super::session::LiveFeishuReadSession;
use super::super::summary::calendar_read_error_reason;
use super::PlannedLiveReads;

pub(super) async fn append_calendar_summary(
    live_summaries: &mut Vec<String>,
    session: &LiveFeishuReadSession,
    planned_reads: PlannedLiveReads,
    lark_open_id_for_tool_reads: &Option<Result<String, &'static str>>,
) {
    if !planned_reads.calendar_free_busy {
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
                Err(error) => live_summaries.push(format!(
                    "工具 feishu.calendar.summarize_my_free_busy｜实时读取降级：{}。",
                    calendar_read_error_reason(error)
                )),
            }
        }
        Some(Err(reason)) => live_summaries.push(format!(
            "工具 feishu.calendar.summarize_my_free_busy｜实时读取降级：{}。",
            reason
        )),
        None => live_summaries.push(
            "工具 feishu.calendar.summarize_my_free_busy｜实时读取降级：用户身份未解析。"
                .to_string(),
        ),
    }
}

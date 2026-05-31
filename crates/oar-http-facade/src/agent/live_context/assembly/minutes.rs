use oar_lark_adapter::{AsyncFeishuMinutesRead, FeishuMinuteReadRequest};

use super::super::minutes_summary::read_my_minutes_summary;
use super::super::session::LiveFeishuReadSession;
use super::super::source_registry::LiveEvidenceResolution;
use super::super::status::LiveFeishuReadStatus;
use super::super::summary::{
    build_minutes_live_summary, degraded_summary, minutes_read_error_reason,
};
use super::PlannedLiveReads;
use crate::agent::tools::AgentReadTool;

pub(super) async fn append_minutes_summaries(
    live_statuses: &mut Vec<LiveFeishuReadStatus>,
    session: &LiveFeishuReadSession,
    evidence_resolution: &mut LiveEvidenceResolution<'_>,
    planned_reads: PlannedLiveReads,
    lark_open_id_for_tool_reads: &Option<Result<String, &'static str>>,
) {
    if evidence_resolution.minutes_refs.is_empty() && !planned_reads.minutes_summary {
        return;
    }

    let mut minutes_client = session.minutes_client();
    if planned_reads.minutes_summary {
        match lark_open_id_for_tool_reads {
            Some(Ok(lark_open_id)) => {
                match read_my_minutes_summary(
                    &mut minutes_client,
                    session.access_token(),
                    lark_open_id,
                )
                .await
                {
                    Ok(summary) => live_statuses.push(LiveFeishuReadStatus::ready_for_tool(
                        AgentReadTool::MinutesSummary,
                        summary,
                    )),
                    Err(error) => live_statuses.push(LiveFeishuReadStatus::degraded_for_tool(
                        AgentReadTool::MinutesSummary,
                        minutes_read_error_reason(error),
                    )),
                }
            }
            Some(Err(reason)) => live_statuses.push(LiveFeishuReadStatus::degraded_for_tool(
                AgentReadTool::MinutesSummary,
                reason,
            )),
            None => live_statuses.push(LiveFeishuReadStatus::degraded_for_tool(
                AgentReadTool::MinutesSummary,
                "用户身份未解析",
            )),
        }
    }

    for (evidence_ref, parsed) in std::mem::take(&mut evidence_resolution.minutes_refs) {
        match minutes_client
            .get_minute_summary(FeishuMinuteReadRequest {
                user_access_token: session.access_token(),
                source_ref: parsed.source_ref(),
            })
            .await
        {
            Ok(summary) => {
                live_statuses.push(LiveFeishuReadStatus::ready(build_minutes_live_summary(
                    evidence_ref,
                    &summary,
                )));
            }
            Err(error) => {
                live_statuses.push(LiveFeishuReadStatus::degraded(degraded_summary(
                    evidence_ref,
                    minutes_read_error_reason(error),
                )));
            }
        }
    }
}

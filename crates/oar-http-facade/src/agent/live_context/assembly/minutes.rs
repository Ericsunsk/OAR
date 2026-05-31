use oar_lark_adapter::{AsyncFeishuMinutesRead, FeishuMinuteReadRequest};

use super::super::session::LiveFeishuReadSession;
use super::super::source_registry::LiveEvidenceResolution;
use super::super::status::LiveFeishuReadStatus;
use super::super::summary::{
    build_minutes_live_summary, degraded_summary, minutes_read_error_reason,
};

pub(super) async fn append_minutes_summaries(
    live_statuses: &mut Vec<LiveFeishuReadStatus>,
    session: &LiveFeishuReadSession,
    evidence_resolution: &mut LiveEvidenceResolution<'_>,
) {
    if evidence_resolution.minutes_refs.is_empty() {
        return;
    }

    let mut minutes_client = session.minutes_client();
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

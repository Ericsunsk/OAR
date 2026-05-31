use oar_lark_adapter::{AsyncFeishuDocRead, FeishuDocReadRequest};

use super::super::session::LiveFeishuReadSession;
use super::super::source_registry::LiveEvidenceResolution;
use super::super::status::LiveFeishuReadStatus;
use super::super::summary::{build_doc_live_summary, degraded_summary, doc_read_error_reason};

pub(super) async fn append_doc_summaries(
    live_statuses: &mut Vec<LiveFeishuReadStatus>,
    session: &LiveFeishuReadSession,
    evidence_resolution: &mut LiveEvidenceResolution<'_>,
) {
    if evidence_resolution.doc_refs.is_empty() {
        return;
    }

    let mut doc_client = session.doc_client();
    for (evidence_ref, parsed) in std::mem::take(&mut evidence_resolution.doc_refs) {
        match doc_client
            .get_doc_summary(FeishuDocReadRequest {
                user_access_token: session.access_token(),
                source_ref: parsed.source_ref(),
            })
            .await
        {
            Ok(summary) => {
                live_statuses.push(LiveFeishuReadStatus::ready(build_doc_live_summary(
                    evidence_ref,
                    &summary,
                )));
            }
            Err(error) => {
                live_statuses.push(LiveFeishuReadStatus::degraded(degraded_summary(
                    evidence_ref,
                    doc_read_error_reason(error),
                )));
            }
        }
    }
}

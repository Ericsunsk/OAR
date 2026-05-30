use oar_lark_adapter::{AsyncFeishuTaskRead, FeishuTaskGetRequest, TaskUserIdType};

use super::super::session::LiveFeishuReadSession;
use super::super::source_registry::LiveEvidenceResolution;
use super::super::summary::{build_task_live_summary, degraded_summary, task_read_error_reason};
use super::super::task_summary::read_my_task_summary;
use super::PlannedLiveReads;

pub(super) async fn append_task_summaries(
    live_summaries: &mut Vec<String>,
    session: &LiveFeishuReadSession,
    evidence_resolution: &mut LiveEvidenceResolution<'_>,
    planned_reads: PlannedLiveReads,
) {
    if evidence_resolution.task_refs.is_empty() && !planned_reads.task_summary {
        return;
    }

    let mut task_client = session.task_client();
    if planned_reads.task_summary {
        match read_my_task_summary(&mut task_client, session.access_token()).await {
            Ok(summary) => live_summaries.push(summary),
            Err(error) => live_summaries.push(format!(
                "工具 feishu.task.summarize_my_tasks｜实时读取降级：{}。",
                task_read_error_reason(error)
            )),
        }
    }

    for (evidence_ref, parsed) in std::mem::take(&mut evidence_resolution.task_refs) {
        match task_client
            .get_task_summary(FeishuTaskGetRequest {
                user_access_token: session.access_token(),
                source_ref: parsed.source_ref,
                user_id_type: TaskUserIdType::OpenId,
            })
            .await
        {
            Ok(summary) => {
                live_summaries.push(build_task_live_summary(evidence_ref, &summary));
            }
            Err(error) => {
                live_summaries.push(degraded_summary(
                    evidence_ref,
                    task_read_error_reason(error),
                ));
            }
        }
    }
}

use std::collections::BTreeSet;

use oar_lark_adapter::{
    AsyncFeishuOkrRead, FeishuOkrBatchGetRequest, OkrReadSnapshot, OkrUserIdType,
};

use super::super::okr_progress_summary::read_my_okr_progress_summary_from_topology;
use super::super::okr_summary::build_my_okr_summary_from_topology;
use super::super::okr_topology::{read_my_okr_topology, OkrTopologyReadOptions};
use super::super::session::LiveFeishuReadSession;
use super::super::source_registry::LiveEvidenceResolution;
use super::super::summary::{
    build_live_summary, okr_read_error_reason, tool_live_degraded_summary,
};
use super::PlannedLiveReads;
use crate::agent::tools::AgentReadTool;

pub(super) async fn append_okr_summaries(
    live_summaries: &mut Vec<String>,
    session: &LiveFeishuReadSession,
    evidence_resolution: &mut LiveEvidenceResolution<'_>,
    planned_reads: PlannedLiveReads,
    lark_open_id_for_tool_reads: &Option<Result<String, &'static str>>,
) {
    if evidence_resolution.okr_refs.is_empty()
        && !planned_reads.okr_summary
        && !planned_reads.okr_progress
    {
        return;
    }

    let mut okr_client = session.okr_client();
    if planned_reads.okr_summary || planned_reads.okr_progress {
        match lark_open_id_for_tool_reads {
            Some(Ok(lark_open_id)) => {
                let topology_result = read_my_okr_topology(
                    &mut okr_client,
                    session.access_token(),
                    lark_open_id,
                    OkrTopologyReadOptions::for_requested_tools(
                        planned_reads.okr_summary,
                        planned_reads.okr_progress,
                    ),
                )
                .await;
                match topology_result {
                    Ok(topology) => {
                        if planned_reads.okr_summary {
                            live_summaries.push(build_my_okr_summary_from_topology(&topology));
                        }
                        if planned_reads.okr_progress {
                            match read_my_okr_progress_summary_from_topology(
                                &mut okr_client,
                                session.access_token(),
                                &topology,
                            )
                            .await
                            {
                                Ok(summary) => live_summaries.push(summary),
                                Err(error) => live_summaries.push(tool_live_degraded_summary(
                                    AgentReadTool::OkrProgress,
                                    okr_read_error_reason(error),
                                )),
                            }
                        }
                    }
                    Err(error) => {
                        push_okr_tool_degraded_summaries(
                            live_summaries,
                            planned_reads.okr_summary,
                            planned_reads.okr_progress,
                            okr_read_error_reason(error),
                        );
                    }
                }
            }
            Some(Err(reason)) => {
                push_okr_tool_degraded_summaries(
                    live_summaries,
                    planned_reads.okr_summary,
                    planned_reads.okr_progress,
                    reason,
                );
            }
            None => {
                push_okr_tool_degraded_summaries(
                    live_summaries,
                    planned_reads.okr_summary,
                    planned_reads.okr_progress,
                    "用户身份未解析",
                );
            }
        }
    }

    if evidence_resolution.okr_refs.is_empty() {
        return;
    }

    let okr_refs = std::mem::take(&mut evidence_resolution.okr_refs);
    let okr_ids = okr_refs
        .iter()
        .map(|(_, parsed)| parsed.okr_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    match okr_client
        .batch_get_okrs(FeishuOkrBatchGetRequest {
            user_access_token: session.access_token(),
            user_id_type: OkrUserIdType::OpenId,
            okr_ids,
            lang: None,
        })
        .await
    {
        Ok(response) => {
            if let Some(data) = response.data {
                let snapshot = OkrReadSnapshot::from_batch_get_data(&data);
                live_summaries.extend(okr_refs.into_iter().map(|(evidence_ref, parsed)| {
                    build_live_summary(evidence_ref, &parsed, &snapshot)
                }));
            } else {
                live_summaries.push("未读取到实时 Feishu 证据：Feishu 返回空数据。".to_string());
            }
        }
        Err(error) => {
            live_summaries.push(format!(
                "未读取到实时 Feishu 证据：{}。",
                okr_read_error_reason(error)
            ));
        }
    }
}

fn push_okr_tool_degraded_summaries(
    live_summaries: &mut Vec<String>,
    include_summary: bool,
    include_progress: bool,
    reason: &str,
) {
    if include_summary {
        live_summaries.push(tool_live_degraded_summary(
            AgentReadTool::OkrSummary,
            reason,
        ));
    }
    if include_progress {
        live_summaries.push(tool_live_degraded_summary(
            AgentReadTool::OkrProgress,
            reason,
        ));
    }
}

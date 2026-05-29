use std::collections::BTreeSet;

use oar_lark_adapter::{
    AsyncFeishuOkrRead, AsyncFeishuTaskRead, FeishuOkrBatchGetRequest, FeishuTaskGetRequest,
    OkrReadSnapshot, OkrUserIdType, TaskUserIdType,
};

use super::request::{AgentEvidenceRefDTO, AgentStreamRequest};
use super::skills::select_skills;
use super::tools::{plan_read_tools, AgentReadTool};
use crate::{AuthenticatedContext, OarHttpFacadeRuntime};

mod authorization;
mod calendar_summary;
mod grant;
mod okr_progress_summary;
mod okr_summary;
mod okr_topology;
mod refs;
mod session;
mod source_registry;
mod summary;
mod task_summary;

use authorization::gate_read_demand_by_scope;
use calendar_summary::read_my_calendar_free_busy_summary;
use okr_progress_summary::read_my_okr_progress_summary_from_topology;
use okr_summary::build_my_okr_summary_from_topology;
use okr_topology::{read_my_okr_topology, OkrTopologyReadOptions};
use session::LiveFeishuReadSession;
use source_registry::resolve_evidence_refs;
use summary::{
    build_live_summary, build_task_live_summary, calendar_read_error_reason, degraded_summary,
    okr_read_error_reason, task_read_error_reason,
};
use task_summary::read_my_task_summary;

const LIVE_EVIDENCE_REF_LIMIT: usize = 4;

pub(crate) async fn inject_live_feishu_context(
    runtime: &OarHttpFacadeRuntime,
    auth_context: &AuthenticatedContext,
    request: &mut AgentStreamRequest,
) {
    let active_skills = select_skills(request);
    request.context.activated_skill_summaries = active_skills
        .iter()
        .map(|skill| skill.prompt_summary())
        .collect();
    let read_tools = plan_read_tools(request);
    let summaries = assemble_live_feishu_summaries(
        runtime,
        auth_context,
        &request.context.evidence_refs,
        &read_tools,
    )
    .await;
    request.context.live_feishu_read_summaries = summaries;
}

async fn assemble_live_feishu_summaries(
    runtime: &OarHttpFacadeRuntime,
    auth_context: &AuthenticatedContext,
    evidence_refs: &[AgentEvidenceRefDTO],
    planned_read_tools: &[AgentReadTool],
) -> Vec<String> {
    if evidence_refs.is_empty() && planned_read_tools.is_empty() {
        return vec![];
    }

    let mut evidence_resolution = resolve_evidence_refs(evidence_refs, LIVE_EVIDENCE_REF_LIMIT);
    let mut read_tools = planned_read_tools.to_vec();

    if evidence_resolution.okr_refs.is_empty()
        && evidence_resolution.task_refs.is_empty()
        && read_tools.is_empty()
    {
        return evidence_resolution.degraded;
    }

    let session_result = LiveFeishuReadSession::open(runtime, auth_context, |scopes| {
        gate_read_demand_by_scope(scopes, &mut evidence_resolution, &mut read_tools)
    })
    .await;
    let session = match session_result {
        Ok(session) => session,
        Err(error) => {
            error.push_degraded(&mut evidence_resolution.degraded);
            return evidence_resolution.degraded;
        }
    };

    let mut live_summaries = Vec::new();
    let should_read_okr_tool = read_tools.contains(&AgentReadTool::FeishuOkrSummarizeMyOkr);
    let should_read_okr_progress_tool =
        read_tools.contains(&AgentReadTool::FeishuOkrSummarizeMyProgress);
    let should_read_task_tool = read_tools.contains(&AgentReadTool::FeishuTaskSummarizeMyTasks);
    let should_read_calendar_tool =
        read_tools.contains(&AgentReadTool::FeishuCalendarSummarizeMyFreeBusy);
    let lark_open_id_for_tool_reads =
        if should_read_okr_tool || should_read_okr_progress_tool || should_read_calendar_tool {
            Some(session.resolve_lark_open_id(auth_context).await)
        } else {
            None
        };

    if !evidence_resolution.okr_refs.is_empty()
        || should_read_okr_tool
        || should_read_okr_progress_tool
    {
        let mut okr_client = session.okr_client();

        if should_read_okr_tool || should_read_okr_progress_tool {
            match &lark_open_id_for_tool_reads {
                Some(Ok(lark_open_id)) => {
                    let topology_result = read_my_okr_topology(
                        &mut okr_client,
                        session.access_token(),
                        lark_open_id,
                        OkrTopologyReadOptions::for_requested_tools(
                            should_read_okr_tool,
                            should_read_okr_progress_tool,
                        ),
                    )
                    .await;
                    match topology_result {
                        Ok(topology) => {
                            if should_read_okr_tool {
                                live_summaries.push(build_my_okr_summary_from_topology(&topology));
                            }
                            if should_read_okr_progress_tool {
                                match read_my_okr_progress_summary_from_topology(
                                    &mut okr_client,
                                    session.access_token(),
                                    &topology,
                                )
                                .await
                                {
                                    Ok(summary) => live_summaries.push(summary),
                                    Err(error) => live_summaries.push(format!(
                                        "工具 feishu.okr.summarize_my_progress｜实时读取降级：{}。",
                                        okr_read_error_reason(error)
                                    )),
                                }
                            }
                        }
                        Err(error) => {
                            push_okr_tool_degraded_summaries(
                                &mut live_summaries,
                                should_read_okr_tool,
                                should_read_okr_progress_tool,
                                okr_read_error_reason(error),
                            );
                        }
                    }
                }
                Some(Err(reason)) => {
                    push_okr_tool_degraded_summaries(
                        &mut live_summaries,
                        should_read_okr_tool,
                        should_read_okr_progress_tool,
                        reason,
                    );
                }
                None => {
                    push_okr_tool_degraded_summaries(
                        &mut live_summaries,
                        should_read_okr_tool,
                        should_read_okr_progress_tool,
                        "用户身份未解析",
                    );
                }
            }
        }

        if !evidence_resolution.okr_refs.is_empty() {
            let okr_ids = evidence_resolution
                .okr_refs
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
                        live_summaries.extend(evidence_resolution.okr_refs.into_iter().map(
                            |(evidence_ref, parsed)| {
                                build_live_summary(evidence_ref, &parsed, &snapshot)
                            },
                        ));
                    } else {
                        live_summaries
                            .push("未读取到实时 Feishu 证据：Feishu 返回空数据。".to_string());
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
    }

    if !evidence_resolution.task_refs.is_empty() || should_read_task_tool {
        let mut task_client = session.task_client();
        if should_read_task_tool {
            match read_my_task_summary(&mut task_client, session.access_token()).await {
                Ok(summary) => live_summaries.push(summary),
                Err(error) => live_summaries.push(format!(
                    "工具 feishu.task.summarize_my_tasks｜实时读取降级：{}。",
                    task_read_error_reason(error)
                )),
            }
        }

        for (evidence_ref, parsed) in evidence_resolution.task_refs {
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

    if should_read_calendar_tool {
        let mut calendar_client = session.calendar_client();
        match &lark_open_id_for_tool_reads {
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

    live_summaries.extend(evidence_resolution.degraded);
    live_summaries
}

fn push_okr_tool_degraded_summaries(
    live_summaries: &mut Vec<String>,
    include_summary: bool,
    include_progress: bool,
    reason: &str,
) {
    if include_summary {
        live_summaries.push(format!(
            "工具 feishu.okr.summarize_my_okr｜实时读取降级：{}。",
            reason
        ));
    }
    if include_progress {
        live_summaries.push(format!(
            "工具 feishu.okr.summarize_my_progress｜实时读取降级：{}。",
            reason
        ));
    }
}

#[cfg(test)]
mod tests;

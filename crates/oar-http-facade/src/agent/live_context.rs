use std::collections::BTreeSet;
use std::time::SystemTime;

use oar_core::storage::postgres::{EncryptedTokenGrantRecord, PostgresTokenGrantRepository};
use oar_lark_adapter::material::read_access_token_from_encrypted_grant;
use oar_lark_adapter::{
    AsyncFeishuOkrRead, AsyncFeishuTaskRead, FeishuCalendarReadClient, FeishuOkrBatchGetRequest,
    FeishuOkrReadClient, FeishuTaskGetRequest, FeishuTaskReadClient, OkrReadSnapshot,
    OkrUserIdType, ReqwestAsyncHttpClient, TaskUserIdType,
};

use super::request::{AgentEvidenceRefDTO, AgentStreamRequest};
use super::skills::select_skills;
use super::tools::{plan_read_tools, AgentReadTool};
use crate::{AuthenticatedContext, OarHttpFacadeRuntime};

mod calendar_summary;
mod grant;
mod okr_progress_summary;
mod okr_summary;
mod okr_topology;
mod refs;
mod source_registry;
mod summary;
mod task_summary;

use calendar_summary::read_my_calendar_free_busy_summary;
use grant::{
    grant_requires_refresh_before_read, live_read_grant_denial_reason,
    refresh_grant_before_live_read, resolve_grant_id_for_user, resolve_lark_open_id_for_grant,
    system_time_to_ms,
};
use okr_progress_summary::read_my_okr_progress_summary_from_topology;
use okr_summary::build_my_okr_summary_from_topology;
use okr_topology::{read_my_okr_topology, OkrTopologyReadOptions};
use source_registry::{gate_evidence_refs_by_scope, resolve_evidence_refs, LiveEvidenceResolution};
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

    let Some(persistence) = runtime
        .feishu_login
        .as_ref()
        .and_then(|login| login.grant_persistence())
    else {
        evidence_resolution
            .degraded
            .push("未读取到实时 Feishu 证据：后端未配置 Feishu 授权存储。".to_string());
        return evidence_resolution.degraded;
    };

    let pool = persistence.pool();
    let grant_id = match resolve_grant_id_for_user(&pool, auth_context).await {
        Ok(grant_id) => grant_id,
        Err(reason) => {
            evidence_resolution
                .degraded
                .push(format!("未读取到实时 Feishu 证据：{}。", reason));
            return evidence_resolution.degraded;
        }
    };

    let token_grant = match PostgresTokenGrantRepository::new(pool.clone())
        .get_by_id(&auth_context.tenant_id, &grant_id)
        .await
    {
        Ok(Some(grant)) => grant,
        Ok(None) => {
            evidence_resolution
                .degraded
                .push("未读取到实时 Feishu 证据：未找到用户授权 grant。".to_string());
            return evidence_resolution.degraded;
        }
        Err(_) => {
            evidence_resolution
                .degraded
                .push("未读取到实时 Feishu 证据：读取授权 grant 失败。".to_string());
            return evidence_resolution.degraded;
        }
    };

    if !gate_grant_and_refs_for_live_read(
        &token_grant,
        persistence.grant_key_id(),
        &mut evidence_resolution,
        &mut read_tools,
    ) {
        return evidence_resolution.degraded;
    }

    let mut token_grant = token_grant;
    let now = SystemTime::now();
    let now_ms = system_time_to_ms(now);
    if grant_requires_refresh_before_read(&token_grant, now_ms) {
        let Some(login) = runtime.feishu_login.as_ref() else {
            evidence_resolution
                .degraded
                .push("未读取到实时 Feishu 证据：后端未配置 Feishu 授权刷新。".to_string());
            return evidence_resolution.degraded;
        };
        token_grant = match refresh_grant_before_live_read(
            pool.clone(),
            login,
            persistence,
            auth_context,
            &token_grant,
            now,
            now_ms,
        )
        .await
        {
            Ok(grant) => grant,
            Err(error) => {
                evidence_resolution.degraded.push(format!(
                    "未读取到实时 Feishu 证据：{}。",
                    error.safe_reason()
                ));
                return evidence_resolution.degraded;
            }
        };
    }

    if !gate_grant_and_refs_for_live_read(
        &token_grant,
        persistence.grant_key_id(),
        &mut evidence_resolution,
        &mut read_tools,
    ) {
        return evidence_resolution.degraded;
    }

    let access_token = match read_access_token_from_encrypted_grant(
        &token_grant.encrypted_oauth_grant,
        persistence.grant_key_material(),
    ) {
        Ok(token) => token,
        Err(_) => {
            evidence_resolution
                .degraded
                .push("未读取到实时 Feishu 证据：授权令牌解密失败。".to_string());
            return evidence_resolution.degraded;
        }
    };

    let open_api_config = runtime
        .feishu_login
        .as_ref()
        .map(|login| login.open_api_config())
        .unwrap_or_default();
    let http_client = match ReqwestAsyncHttpClient::with_config(&open_api_config) {
        Ok(client) => client,
        Err(_) => {
            evidence_resolution
                .degraded
                .push("未读取到实时 Feishu 证据：Feishu HTTP 客户端初始化失败。".to_string());
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
            Some(resolve_lark_open_id_for_grant(&pool, auth_context, &token_grant).await)
        } else {
            None
        };

    if !evidence_resolution.okr_refs.is_empty()
        || should_read_okr_tool
        || should_read_okr_progress_tool
    {
        let mut okr_client = FeishuOkrReadClient::new(open_api_config.clone(), http_client.clone());

        if should_read_okr_tool || should_read_okr_progress_tool {
            match &lark_open_id_for_tool_reads {
                Some(Ok(lark_open_id)) => {
                    let topology_result = read_my_okr_topology(
                        &mut okr_client,
                        access_token.clone(),
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
                                    access_token.clone(),
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
                    user_access_token: access_token.clone(),
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
        let mut task_client =
            FeishuTaskReadClient::new(open_api_config.clone(), http_client.clone());
        if should_read_task_tool {
            match read_my_task_summary(&mut task_client, access_token.clone()).await {
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
                    user_access_token: access_token.clone(),
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
        let mut calendar_client = FeishuCalendarReadClient::new(open_api_config, http_client);
        match &lark_open_id_for_tool_reads {
            Some(Ok(lark_open_id)) => {
                match read_my_calendar_free_busy_summary(
                    &mut calendar_client,
                    access_token.clone(),
                    lark_open_id,
                    now,
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

fn gate_read_tools_by_scope(
    scopes: &[String],
    read_tools: &mut Vec<AgentReadTool>,
    degraded: &mut Vec<String>,
) {
    read_tools.retain(|tool| {
        let spec = tool.spec();
        let required_scopes = match spec.required_feishu_scopes() {
            Ok(scopes) => scopes,
            Err(error) => {
                degraded.push(format!(
                    "工具 {}｜实时读取降级：{}。",
                    spec.name,
                    error.safe_reason()
                ));
                return false;
            }
        };
        let missing = required_scopes
            .iter()
            .filter_map(|required| {
                let required = required.as_str();
                if scopes.iter().any(|scope| scope.trim() == required) {
                    None
                } else {
                    Some(required)
                }
            })
            .collect::<Vec<_>>();
        if missing.is_empty() {
            return true;
        }
        degraded.push(format!(
            "工具 {}｜实时读取降级：授权缺少 {}。",
            spec.name,
            missing.join("、")
        ));
        false
    });
}

fn gate_grant_and_refs_for_live_read<'a>(
    token_grant: &EncryptedTokenGrantRecord,
    expected_grant_key_id: &str,
    evidence_resolution: &mut LiveEvidenceResolution<'a>,
    read_tools: &mut Vec<AgentReadTool>,
) -> bool {
    if let Some(reason) = live_read_grant_denial_reason(token_grant) {
        evidence_resolution
            .degraded
            .push(format!("未读取到实时 Feishu 证据：{}。", reason));
        return false;
    }

    if token_grant.oauth_grant_key_id != expected_grant_key_id {
        evidence_resolution
            .degraded
            .push("未读取到实时 Feishu 证据：授权密钥版本不匹配。".to_string());
        return false;
    }

    gate_evidence_refs_by_scope(&token_grant.scopes, evidence_resolution);
    gate_read_tools_by_scope(
        &token_grant.scopes,
        read_tools,
        &mut evidence_resolution.degraded,
    );
    !(evidence_resolution.okr_refs.is_empty()
        && evidence_resolution.task_refs.is_empty()
        && read_tools.is_empty())
}

#[cfg(test)]
mod tests;

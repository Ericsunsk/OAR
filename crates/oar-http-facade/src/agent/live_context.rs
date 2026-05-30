use super::request::AgentStreamRequest;
use super::skills::select_skills;
use super::tools::plan_read_tools;
use crate::{AuthenticatedContext, OarHttpFacadeRuntime};

mod assembly;
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

use assembly::assemble_live_feishu_summaries;

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

#[cfg(test)]
mod tests;

use super::activation::plan_agent_skill_activation;
use super::request::AgentStreamRequest;
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
pub(in crate::agent) mod status;
mod summary;
mod task_summary;

use assembly::assemble_live_feishu_statuses;

const LIVE_EVIDENCE_REF_LIMIT: usize = 4;

pub(crate) async fn inject_live_feishu_context(
    runtime: &OarHttpFacadeRuntime,
    auth_context: &AuthenticatedContext,
    request: &mut AgentStreamRequest,
) {
    let activation_plan = plan_agent_skill_activation(request);
    request.context.activated_skill_summaries = activation_plan.activated_skill_summaries();
    request.context.activated_skill_statuses = activation_plan.activated_skill_statuses();
    let statuses = assemble_live_feishu_statuses(
        runtime,
        auth_context,
        &request.context.evidence_refs,
        activation_plan.read_tools(),
    )
    .await;
    request.context.live_feishu_read_summaries = statuses
        .iter()
        .map(|status| status.summary.clone())
        .collect();
    request.context.live_feishu_read_statuses = statuses;
}

#[cfg(test)]
mod tests;

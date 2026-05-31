use oar_lark_adapter::{
    AsyncFeishuOkrRead, FeishuOkrProgressListRequest, FeishuOkrReadError, OkrDepartmentIdType,
    OkrReadProgressPage, OkrUserIdType, SecretString,
};

use super::okr_topology::OkrTopologyRead;
use super::summary::tool_live_label;
use crate::agent::tools::AgentReadTool;

mod aggregation;
mod rendering;
mod targets;
mod text;

use aggregation::OkrProgressAggregation;
use rendering::build_okr_progress_live_summary;
use targets::{discover_progress_targets, CYCLE_EXPAND_LIMIT};

#[cfg(test)]
use oar_lark_adapter::OkrReadProgressRecord;
#[cfg(test)]
use targets::{ProgressTarget, ProgressTargetKind};
#[cfg(test)]
use text::short_title;

const PROGRESS_PAGE_SIZE: u32 = 20;

pub(super) async fn read_my_okr_progress_summary_from_topology<C>(
    okr_client: &mut C,
    access_token: SecretString,
    topology: &OkrTopologyRead,
) -> Result<String, FeishuOkrReadError>
where
    C: AsyncFeishuOkrRead,
{
    let OkrTopologyRead::Snapshot(snapshot) = topology else {
        let tool_label = tool_live_label(AgentReadTool::OkrProgress);
        return Ok(format!("{tool_label}｜实时：Feishu 返回空数据。"));
    };
    let mut aggregation = OkrProgressAggregation {
        cycles_total: snapshot.cycles.len(),
        cycle_pages_with_more: usize::from(snapshot.has_more_cycles),
        skipped_cycles: snapshot.cycles.len().saturating_sub(CYCLE_EXPAND_LIMIT),
        ..OkrProgressAggregation::default()
    };

    if snapshot.cycles.is_empty() {
        return Ok(build_okr_progress_live_summary(&aggregation));
    }

    let targets = discover_progress_targets(snapshot, &mut aggregation);

    for target in targets {
        let progress_response = okr_client
            .list_progress(FeishuOkrProgressListRequest {
                user_access_token: access_token.clone(),
                user_id_type: OkrUserIdType::OpenId,
                target: target.to_adapter_target(),
                page_size: Some(PROGRESS_PAGE_SIZE),
                page_token: None,
                department_id_type: OkrDepartmentIdType::OpenDepartmentId,
            })
            .await?;
        let page = progress_response
            .data
            .as_ref()
            .map(OkrReadProgressPage::from_progress_list_data)
            .unwrap_or_else(|| OkrReadProgressPage {
                progress_records: vec![],
                next_page_token: None,
                has_more: false,
            });
        aggregation.add_progress_page(&target, &page);
    }

    Ok(build_okr_progress_live_summary(&aggregation))
}

#[cfg(test)]
mod tests;

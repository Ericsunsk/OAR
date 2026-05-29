use oar_lark_adapter::{
    AsyncFeishuOkrRead, FeishuOkrCycleListRequest, FeishuOkrCycleObjectivesListRequest,
    FeishuOkrObjectiveKeyResultsListRequest, FeishuOkrReadError, OkrReadCyclesPage,
    OkrReadKeyResultsPage, OkrReadObjectivesPage, OkrUserIdType, SecretString,
};

use super::{
    OkrTopologyCycle, OkrTopologyKeyResults, OkrTopologyRead, OkrTopologyReadOptions,
    OkrTopologySnapshot,
};

pub(in crate::agent::live_context) async fn read_my_okr_topology<C>(
    okr_client: &mut C,
    access_token: SecretString,
    lark_open_id: &str,
    options: OkrTopologyReadOptions,
) -> Result<OkrTopologyRead, FeishuOkrReadError>
where
    C: AsyncFeishuOkrRead,
{
    let response = okr_client
        .list_cycles(FeishuOkrCycleListRequest {
            user_access_token: access_token.clone(),
            user_id_type: OkrUserIdType::OpenId,
            user_id: lark_open_id.to_string(),
            page_size: Some(options.cycle_page_size),
            page_token: None,
            lang: None,
        })
        .await?;
    let Some(data) = response.data else {
        return Ok(OkrTopologyRead::EmptyData);
    };

    let cycles_page = OkrReadCyclesPage::from_cycle_list_data(&data);
    let mut snapshot = OkrTopologySnapshot {
        cycles: Vec::with_capacity(cycles_page.cycles.len()),
        has_more_cycles: cycles_page.has_more,
    };

    for (index, cycle) in cycles_page.cycles.into_iter().enumerate() {
        let mut topology_cycle = OkrTopologyCycle {
            cycle,
            objectives: None,
            objectives_has_more: false,
            key_results: Vec::new(),
        };

        if index >= options.cycle_expand_limit {
            snapshot.cycles.push(topology_cycle);
            continue;
        }

        let Some(cycle_id) = topology_cycle
            .cycle
            .cycle_id
            .as_deref()
            .filter(|id| !id.trim().is_empty())
            .map(ToOwned::to_owned)
        else {
            snapshot.cycles.push(topology_cycle);
            continue;
        };

        let objectives_response = okr_client
            .list_cycle_objectives(FeishuOkrCycleObjectivesListRequest {
                user_access_token: access_token.clone(),
                user_id_type: OkrUserIdType::OpenId,
                cycle_id: cycle_id.clone(),
                page_size: Some(options.objective_page_size),
                page_token: None,
                lang: None,
            })
            .await?;
        let Some(objectives_data) = objectives_response.data else {
            snapshot.cycles.push(topology_cycle);
            continue;
        };

        let objectives_page =
            OkrReadObjectivesPage::from_cycle_objectives_list_data(&cycle_id, &objectives_data);
        for objective in objectives_page
            .objectives
            .iter()
            .take(options.objective_key_result_limit)
        {
            let Some(objective_id) = objective
                .objective_id
                .as_deref()
                .filter(|id| !id.trim().is_empty())
                .map(ToOwned::to_owned)
            else {
                continue;
            };

            let key_results_response = okr_client
                .list_objective_key_results(FeishuOkrObjectiveKeyResultsListRequest {
                    user_access_token: access_token.clone(),
                    user_id_type: OkrUserIdType::OpenId,
                    objective_id: objective_id.clone(),
                    page_size: Some(options.key_result_page_size),
                    page_token: None,
                    lang: None,
                })
                .await?;
            if let Some(key_results_data) = key_results_response.data {
                let page = OkrReadKeyResultsPage::from_objective_key_results_list_data(
                    objective_id.clone(),
                    &key_results_data,
                );
                topology_cycle.key_results.push(OkrTopologyKeyResults {
                    objective_id,
                    krs: page.krs,
                    has_more: page.has_more,
                });
            }
        }

        topology_cycle.objectives_has_more = objectives_page.has_more;
        topology_cycle.objectives = Some(objectives_page.objectives);
        snapshot.cycles.push(topology_cycle);
    }

    Ok(OkrTopologyRead::Snapshot(snapshot))
}

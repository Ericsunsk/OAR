use oar_lark_adapter::{
    AsyncFeishuOkrRead, FeishuOkrCycleListRequest, FeishuOkrCycleObjectivesListRequest,
    FeishuOkrObjectiveKeyResultsListRequest, FeishuOkrReadError, OkrReadCycle, OkrReadCyclesPage,
    OkrReadKeyResult, OkrReadKeyResultsPage, OkrReadObjective, OkrReadObjectivesPage,
    OkrUserIdType, SecretString,
};

const SUMMARY_PAGE_SIZE: u32 = 100;
const PROGRESS_PAGE_SIZE: u32 = 50;
const CYCLE_EXPAND_LIMIT: usize = 3;
const SUMMARY_OBJECTIVE_KEY_RESULT_LIMIT: usize = 8;
const PROGRESS_OBJECTIVE_KEY_RESULT_LIMIT: usize = 10;

#[derive(Clone, Copy)]
pub(super) struct OkrTopologyReadOptions {
    cycle_page_size: u32,
    objective_page_size: u32,
    key_result_page_size: u32,
    cycle_expand_limit: usize,
    objective_key_result_limit: usize,
}

impl OkrTopologyReadOptions {
    pub(super) fn for_requested_tools(read_summary: bool, read_progress: bool) -> Self {
        match (read_summary, read_progress) {
            (true, true) => Self {
                cycle_page_size: SUMMARY_PAGE_SIZE,
                objective_page_size: SUMMARY_PAGE_SIZE,
                key_result_page_size: SUMMARY_PAGE_SIZE,
                cycle_expand_limit: CYCLE_EXPAND_LIMIT,
                objective_key_result_limit: PROGRESS_OBJECTIVE_KEY_RESULT_LIMIT,
            },
            (true, false) => Self {
                cycle_page_size: SUMMARY_PAGE_SIZE,
                objective_page_size: SUMMARY_PAGE_SIZE,
                key_result_page_size: SUMMARY_PAGE_SIZE,
                cycle_expand_limit: CYCLE_EXPAND_LIMIT,
                objective_key_result_limit: SUMMARY_OBJECTIVE_KEY_RESULT_LIMIT,
            },
            (false, true) => Self {
                cycle_page_size: PROGRESS_PAGE_SIZE,
                objective_page_size: PROGRESS_PAGE_SIZE,
                key_result_page_size: PROGRESS_PAGE_SIZE,
                cycle_expand_limit: CYCLE_EXPAND_LIMIT,
                objective_key_result_limit: PROGRESS_OBJECTIVE_KEY_RESULT_LIMIT,
            },
            (false, false) => Self {
                cycle_page_size: PROGRESS_PAGE_SIZE,
                objective_page_size: PROGRESS_PAGE_SIZE,
                key_result_page_size: PROGRESS_PAGE_SIZE,
                cycle_expand_limit: 0,
                objective_key_result_limit: 0,
            },
        }
    }
}

#[derive(Clone)]
pub(super) enum OkrTopologyRead {
    EmptyData,
    Snapshot(OkrTopologySnapshot),
}

#[derive(Clone, Default)]
pub(super) struct OkrTopologySnapshot {
    pub(super) cycles: Vec<OkrTopologyCycle>,
    pub(super) has_more_cycles: bool,
}

#[derive(Clone)]
pub(super) struct OkrTopologyCycle {
    pub(super) cycle: OkrReadCycle,
    pub(super) objectives: Option<Vec<OkrReadObjective>>,
    pub(super) objectives_has_more: bool,
    pub(super) key_results: Vec<OkrTopologyKeyResults>,
}

impl OkrTopologyCycle {
    pub(super) fn stable_cycle_id(&self) -> Option<&str> {
        self.cycle
            .cycle_id
            .as_deref()
            .filter(|id| !id.trim().is_empty())
    }

    pub(super) fn key_results_for_objective(
        &self,
        objective_id: &str,
    ) -> Option<&OkrTopologyKeyResults> {
        self.key_results
            .iter()
            .find(|entry| entry.objective_id == objective_id)
    }
}

#[derive(Clone)]
pub(super) struct OkrTopologyKeyResults {
    pub(super) objective_id: String,
    pub(super) krs: Vec<OkrReadKeyResult>,
    pub(super) has_more: bool,
}

pub(super) async fn read_my_okr_topology<C>(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::live_context::okr_progress_summary::read_my_okr_progress_summary_from_topology;
    use crate::agent::live_context::okr_summary::build_my_okr_summary_from_topology;
    use async_trait::async_trait;
    use oar_lark_adapter::{
        FeishuOkrBatchGetRequest, FeishuOkrBatchGetResponse, FeishuOkrCycleListData,
        FeishuOkrCycleListResponse, FeishuOkrCycleObjectivesListData,
        FeishuOkrCycleObjectivesListResponse, FeishuOkrObjectiveKeyResultsListData,
        FeishuOkrObjectiveKeyResultsListResponse, FeishuOkrProgressListData,
        FeishuOkrProgressListRequest, FeishuOkrProgressListResponse,
    };
    use serde_json::json;

    #[tokio::test]
    async fn topology_read_is_shared_by_summary_and_progress_overlay() {
        let mut client = FakeOkrClient::default();
        let topology = read_my_okr_topology(
            &mut client,
            SecretString::new("secret-access-token"),
            "ou_user_raw",
            OkrTopologyReadOptions::for_requested_tools(true, true),
        )
        .await
        .expect("topology");

        assert_eq!(client.cycle_calls, 1);
        assert_eq!(client.objective_calls, 1);
        assert_eq!(client.key_result_calls, 1);
        assert_eq!(client.progress_calls, 0);

        let summary = build_my_okr_summary_from_topology(&topology);
        assert!(summary.contains("2026 H1"));
        assert!(summary.contains("1 个 Objective、1 个 KR"));
        assert!(!summary.contains("cycle_raw_secret"));
        assert!(!summary.contains("obj_raw_secret"));
        assert!(!summary.contains("kr_raw_secret"));
        assert!(!summary.contains("next_page_token"));
        assert!(!summary.contains("secret-access-token"));

        let progress = read_my_okr_progress_summary_from_topology(
            &mut client,
            SecretString::new("secret-access-token"),
            &topology,
        )
        .await
        .expect("progress");

        assert_eq!(client.cycle_calls, 1);
        assert_eq!(client.objective_calls, 1);
        assert_eq!(client.key_result_calls, 1);
        assert_eq!(client.progress_calls, 2);
        assert!(progress.contains("记录 2"));
        assert!(!progress.contains("obj_raw_secret"));
        assert!(!progress.contains("kr_raw_secret"));
        assert!(!progress.contains("progress_record_raw_id"));
        assert!(!progress.contains("private progress body"));
        assert!(!progress.contains("secret-access-token"));
    }

    #[tokio::test]
    async fn summary_projection_does_not_fall_back_to_raw_cycle_id() {
        let mut client = FakeOkrClient {
            unnamed_cycle: true,
            ..FakeOkrClient::default()
        };
        let topology = read_my_okr_topology(
            &mut client,
            SecretString::new("secret-access-token"),
            "ou_user_raw",
            OkrTopologyReadOptions::for_requested_tools(true, false),
        )
        .await
        .expect("topology");

        let summary = build_my_okr_summary_from_topology(&topology);

        assert!(summary.contains("未命名周期"));
        assert!(!summary.contains("cycle_raw_secret"));
    }

    #[derive(Default)]
    struct FakeOkrClient {
        cycle_calls: usize,
        objective_calls: usize,
        key_result_calls: usize,
        progress_calls: usize,
        unnamed_cycle: bool,
    }

    #[async_trait]
    impl AsyncFeishuOkrRead for FakeOkrClient {
        async fn batch_get_okrs(
            &mut self,
            _request: FeishuOkrBatchGetRequest,
        ) -> Result<FeishuOkrBatchGetResponse, FeishuOkrReadError> {
            Err(FeishuOkrReadError::InvalidRequest)
        }

        async fn list_cycles(
            &mut self,
            _request: FeishuOkrCycleListRequest,
        ) -> Result<FeishuOkrCycleListResponse, FeishuOkrReadError> {
            self.cycle_calls += 1;
            Ok(FeishuOkrCycleListResponse {
                code: 0,
                msg: None,
                data: Some(
                    serde_json::from_value::<FeishuOkrCycleListData>(json!({
                        "items": [{
                            "id": "cycle_raw_secret",
                            "name": if self.unnamed_cycle { "" } else { "2026 H1" }
                        }],
                        "next_page_token": "raw_cycle_page_token",
                        "has_more": false
                    }))
                    .expect("cycle data"),
                ),
            })
        }

        async fn list_cycle_objectives(
            &mut self,
            _request: FeishuOkrCycleObjectivesListRequest,
        ) -> Result<FeishuOkrCycleObjectivesListResponse, FeishuOkrReadError> {
            self.objective_calls += 1;
            Ok(FeishuOkrCycleObjectivesListResponse {
                code: 0,
                msg: None,
                data: Some(
                    serde_json::from_value::<FeishuOkrCycleObjectivesListData>(json!({
                        "items": [{
                            "id": "obj_raw_secret",
                            "content": {"text": "Launch integration"},
                            "progress_rate": {"percent": "60", "status": "normal"},
                            "last_updated_time": "2026-05-20T10:00:00Z"
                        }],
                        "next_page_token": "raw_objective_page_token",
                        "has_more": false
                    }))
                    .expect("objective data"),
                ),
            })
        }

        async fn list_objective_key_results(
            &mut self,
            _request: FeishuOkrObjectiveKeyResultsListRequest,
        ) -> Result<FeishuOkrObjectiveKeyResultsListResponse, FeishuOkrReadError> {
            self.key_result_calls += 1;
            Ok(FeishuOkrObjectiveKeyResultsListResponse {
                code: 0,
                msg: None,
                data: Some(
                    serde_json::from_value::<FeishuOkrObjectiveKeyResultsListData>(json!({
                        "items": [{
                            "id": "kr_raw_secret",
                            "content": {"text": "Ship safely"},
                            "progress_rate": {"percent": "70", "status": "risk"},
                            "last_updated_time": "2026-05-21T10:00:00Z"
                        }],
                        "next_page_token": "raw_kr_page_token",
                        "has_more": false
                    }))
                    .expect("kr data"),
                ),
            })
        }

        async fn list_progress(
            &mut self,
            _request: FeishuOkrProgressListRequest,
        ) -> Result<FeishuOkrProgressListResponse, FeishuOkrReadError> {
            self.progress_calls += 1;
            Ok(FeishuOkrProgressListResponse {
                code: 0,
                msg: None,
                data: Some(
                    serde_json::from_value::<FeishuOkrProgressListData>(json!({
                        "progress_list": [{
                            "progress_id": "progress_record_raw_id",
                            "modify_time": "2026-05-22T10:00:00Z",
                            "content": {"text": "private progress body"},
                            "progress_rate": {"percent": "80", "status": "normal"}
                        }],
                        "next_page_token": "raw_progress_page_token",
                        "has_more": false
                    }))
                    .expect("progress data"),
                ),
            })
        }
    }
}

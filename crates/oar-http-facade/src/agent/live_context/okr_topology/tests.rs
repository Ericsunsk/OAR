use super::*;
use crate::agent::live_context::okr_progress_summary::read_my_okr_progress_summary_from_topology;
use crate::agent::live_context::okr_summary::build_my_okr_summary_from_topology;
use async_trait::async_trait;
use oar_lark_adapter::{
    AsyncFeishuOkrRead, FeishuOkrBatchGetRequest, FeishuOkrBatchGetResponse,
    FeishuOkrCycleListData, FeishuOkrCycleListRequest, FeishuOkrCycleListResponse,
    FeishuOkrCycleObjectivesListData, FeishuOkrCycleObjectivesListRequest,
    FeishuOkrCycleObjectivesListResponse, FeishuOkrObjectiveKeyResultsListData,
    FeishuOkrObjectiveKeyResultsListRequest, FeishuOkrObjectiveKeyResultsListResponse,
    FeishuOkrProgressListData, FeishuOkrProgressListRequest, FeishuOkrProgressListResponse,
    FeishuOkrReadError, SecretString,
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

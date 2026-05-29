use async_trait::async_trait;
use serde_json::json;

use crate::config::FeishuOpenApiConfig;
use crate::oauth::{AsyncHttpClient, HttpClient, HttpClientFailure, HttpRequest, HttpResponse};
use crate::okr::{
    AsyncFeishuOkrRead, FeishuOkrBatchGetRequest, FeishuOkrCycleListRequest,
    FeishuOkrCycleObjectivesListRequest, FeishuOkrObjectiveKeyResultsListRequest,
    FeishuOkrReadClient, FeishuOkrReadError, OkrReadCyclesPage, OkrReadKeyResultsPage,
    OkrReadObjectivesPage, OkrReadSnapshot, OkrUserIdType,
};
use crate::redaction::SecretString;

#[derive(Clone)]
struct FakeHttpClient {
    response: Option<HttpResponse>,
    error: Option<HttpClientFailure>,
    request: Option<HttpRequest>,
}

impl FakeHttpClient {
    fn from_response(response: HttpResponse) -> Self {
        Self {
            response: Some(response),
            error: None,
            request: None,
        }
    }

    fn from_error(error: HttpClientFailure) -> Self {
        Self {
            response: None,
            error: Some(error),
            request: None,
        }
    }
}

impl HttpClient for FakeHttpClient {
    fn post_json(&mut self, request: HttpRequest) -> Result<HttpResponse, HttpClientFailure> {
        self.request = Some(request);
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        Ok(self.response.clone().expect("response exists"))
    }
}

#[derive(Clone)]
struct AsyncFakeHttpClient {
    response: HttpResponse,
}

#[async_trait]
impl AsyncHttpClient for AsyncFakeHttpClient {
    async fn post_json(
        &mut self,
        _request: HttpRequest,
    ) -> Result<HttpResponse, HttpClientFailure> {
        Ok(self.response.clone())
    }
}

fn sample_request() -> FeishuOkrBatchGetRequest {
    FeishuOkrBatchGetRequest {
        user_access_token: SecretString::new("u-very-secret-token"),
        user_id_type: OkrUserIdType::OpenId,
        okr_ids: vec!["okr_1".to_string(), "okr_2".to_string()],
        lang: Some("zh_cn".to_string()),
    }
}

fn sample_cycle_list_request() -> FeishuOkrCycleListRequest {
    FeishuOkrCycleListRequest {
        user_access_token: SecretString::new("u-very-secret-token"),
        user_id_type: OkrUserIdType::OpenId,
        user_id: "ou_user_1".to_string(),
        page_size: Some(100),
        page_token: Some("next token/1".to_string()),
        lang: Some("zh_cn".to_string()),
    }
}

fn sample_cycle_objectives_request() -> FeishuOkrCycleObjectivesListRequest {
    FeishuOkrCycleObjectivesListRequest {
        user_access_token: SecretString::new("u-very-secret-token"),
        user_id_type: OkrUserIdType::OpenId,
        cycle_id: "cycle 2026/05".to_string(),
        page_size: Some(100),
        page_token: Some("objective token/1".to_string()),
        lang: Some("zh_cn".to_string()),
    }
}

fn sample_objective_key_results_request() -> FeishuOkrObjectiveKeyResultsListRequest {
    FeishuOkrObjectiveKeyResultsListRequest {
        user_access_token: SecretString::new("u-very-secret-token"),
        user_id_type: OkrUserIdType::OpenId,
        objective_id: "obj/1?x".to_string(),
        page_size: Some(100),
        page_token: Some("kr token/1".to_string()),
        lang: Some("zh_cn".to_string()),
    }
}

#[test]
fn batch_get_success_response_parses() {
    let response = HttpResponse::new(
        200,
        json!({
            "code": 0,
            "msg": "ok",
            "data": {
                "okr_list": [
                    {
                        "id":"okr_1",
                        "period_id":"period_2026_q2",
                        "name":"A",
                        "permission":1,
                        "confirm_status":2,
                        "objective_list":[
                            {
                                "id":"obj_1",
                                "content":"grow x",
                                "permission":3,
                                "score":88.5,
                                "weight":60,
                                "progress_rate":{"percent":73,"status":"normal"},
                                "progress_record_list":[{"id":"pr_1"},{"id":"pr_2"}],
                                "last_updated_time":"2026-05-20T10:00:00Z",
                                "deadline":"2026-06-30",
                                "kr_list":[
                                    {
                                        "id":"kr_1",
                                        "content":"ship y",
                                        "score":95,
                                        "kr_weight":20,
                                        "weight":0.5,
                                        "progress_rate":{"percent":80,"status":"normal"},
                                        "progress_record_list":[{"id":"kpr_1"}],
                                        "last_updated_time":"2026-05-21T10:00:00Z",
                                        "deadline":"2026-06-25"
                                    }
                                ]
                            }
                        ]
                    },
                    {"id":"okr_2","name":"B","objective_list":[]}
                ]
            }
        })
        .to_string(),
    );
    let mut client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(response),
    );
    let parsed = client.batch_get_okrs(sample_request()).expect("success");
    assert_eq!(parsed.code, 0);
    let data = parsed.data.expect("data");
    assert_eq!(data.okr_list.len(), 2);
    assert_eq!(data.okr_list[0].id.as_deref(), Some("okr_1"));
    assert_eq!(data.okr_list[0].permission.as_deref(), Some("1"));
    assert_eq!(data.okr_list[0].confirm_status.as_deref(), Some("2"));
    assert_eq!(
        data.okr_list[0].objective_list[0].score.as_deref(),
        Some("88.5")
    );
    assert_eq!(
        data.okr_list[0].objective_list[0].weight.as_deref(),
        Some("60")
    );
    assert_eq!(
        data.okr_list[0].objective_list[0]
            .progress_rate
            .as_ref()
            .and_then(|x| x.percent.as_deref()),
        Some("73")
    );
    assert_eq!(
        data.okr_list[0].objective_list[0].kr_list[0].progress_record_list[0]
            .id
            .as_deref(),
        Some("kpr_1")
    );
    assert_eq!(
        data.okr_list[0].objective_list[0].kr_list[0]
            .kr_weight
            .as_deref(),
        Some("20")
    );
    assert_eq!(
        data.okr_list[0].objective_list[0].kr_list[0]
            .weight
            .as_deref(),
        Some("0.5")
    );
}

#[test]
fn batch_get_typed_model_tolerates_missing_optional_fields_and_normalizes() {
    let response = HttpResponse::new(
        200,
        json!({
            "code": 0,
            "data": {
                "okr_list": [
                    {
                        "id":"okr_1",
                        "period_id":"period_2026_q2",
                        "name":"north star",
                        "objective_list":[
                            {
                                "id":"obj_1",
                                "content":"grow x",
                                "progress_rate":{"percent":73,"status":"normal"},
                                "progress_record_list":[{"id":"pr_1"},{"id":"pr_2"}],
                                "progress_rate_percent_last_updated_time":"1780000000000",
                                "progress_record_last_updated_time":"1781000000000",
                                "progress_report_last_updated_time":"0",
                                "deadline":"2026-06-30",
                                "kr_list":[
                                    {
                                        "id":"kr_1",
                                        "content":"ship y",
                                        "progress_rate":{"percent":80,"status":"normal"},
                                        "progress_record_list":[{"id":"kpr_1"}],
                                        "progress_rate_percent_last_updated_time":"1780000000001",
                                        "progress_rate_status_last_updated_time":"1781000000001",
                                        "progress_record_last_updated_time":"1782000000001",
                                        "deadline":"2026-06-25"
                                    },
                                    {
                                        "id":"kr_2",
                                        "content":"no rate or record fields"
                                    }
                                ]
                            }
                        ]
                    }
                ]
            }
        })
        .to_string(),
    );
    let mut client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(response),
    );
    let parsed = client.batch_get_okrs(sample_request()).expect("success");
    let data = parsed.data.expect("data");
    let snapshot = OkrReadSnapshot::from_batch_get_data(&data);
    assert_eq!(snapshot.okrs.len(), 1);
    assert_eq!(snapshot.okrs[0].okr_id.as_deref(), Some("okr_1"));
    assert_eq!(
        snapshot.okrs[0].period_id.as_deref(),
        Some("period_2026_q2")
    );
    assert_eq!(snapshot.okrs[0].okr_name.as_deref(), Some("north star"));
    assert_eq!(snapshot.okrs[0].objectives.len(), 1);
    assert_eq!(
        snapshot.okrs[0].objectives[0].progress_record_ids,
        vec!["pr_1".to_string(), "pr_2".to_string()]
    );
    assert_eq!(
        snapshot.okrs[0].objectives[0].last_updated_time.as_deref(),
        Some("1781000000000")
    );
    assert_eq!(snapshot.okrs[0].objectives[0].krs.len(), 2);
    assert_eq!(
        snapshot.okrs[0].objectives[0].krs[0].kr_id.as_deref(),
        Some("kr_1")
    );
    assert_eq!(
        snapshot.okrs[0].objectives[0].krs[0].progress_record_ids,
        vec!["kpr_1".to_string()]
    );
    assert_eq!(
        snapshot.okrs[0].objectives[0].krs[0]
            .last_updated_time
            .as_deref(),
        Some("1782000000001")
    );
    assert!(snapshot.okrs[0].objectives[0].krs[1].progress.is_none());
    assert!(snapshot.okrs[0].objectives[0].krs[1].status.is_none());
    assert!(snapshot.okrs[0].objectives[0].krs[1]
        .progress_record_ids
        .is_empty());
}

#[test]
fn batch_get_accepts_legacy_id_aliases_and_preserves_non_epoch_update_time() {
    let response = HttpResponse::new(
        200,
        json!({
            "code": 0,
            "data": {
                "okr_list": [{
                    "okr_id":"okr_alias",
                    "objective_list":[{
                        "objective_id":"obj_alias",
                        "progress_report_last_updated_time":"2026-05-21T10:00:00Z",
                        "kr_list":[{
                            "kr_id":"kr_alias",
                            "progress_record_last_updated_time":"2026-05-22T10:00:00Z"
                        }]
                    }]
                }]
            }
        })
        .to_string(),
    );
    let mut client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(response),
    );
    let parsed = client.batch_get_okrs(sample_request()).expect("success");
    let snapshot = OkrReadSnapshot::from_batch_get_data(&parsed.data.expect("data"));

    assert_eq!(snapshot.okrs[0].okr_id.as_deref(), Some("okr_alias"));
    assert_eq!(
        snapshot.okrs[0].objectives[0].objective_id.as_deref(),
        Some("obj_alias")
    );
    assert_eq!(
        snapshot.okrs[0].objectives[0].last_updated_time.as_deref(),
        Some("2026-05-21T10:00:00Z")
    );
    assert_eq!(
        snapshot.okrs[0].objectives[0].krs[0].kr_id.as_deref(),
        Some("kr_alias")
    );
    assert_eq!(
        snapshot.okrs[0].objectives[0].krs[0]
            .last_updated_time
            .as_deref(),
        Some("2026-05-22T10:00:00Z")
    );
}

#[test]
fn batch_get_accepts_numeric_progress_status() {
    let response = HttpResponse::new(
        200,
        json!({
            "code": 0,
            "data": {
                "okr_list": [{
                    "id":"okr_1",
                    "objective_list":[{
                        "id":"obj_1",
                        "permission":{"unexpected":true},
                        "progress_rate":{"percent":50.5,"status":1},
                        "kr_list":[]
                    }]
                }]
            }
        })
        .to_string(),
    );
    let mut client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(response),
    );
    let parsed = client.batch_get_okrs(sample_request()).expect("success");
    let objective = &parsed.data.expect("data").okr_list[0].objective_list[0];
    assert!(objective.permission.is_none());
    assert_eq!(
        objective
            .progress_rate
            .as_ref()
            .and_then(|x| x.percent.as_deref()),
        Some("50.5")
    );
    assert_eq!(
        objective
            .progress_rate
            .as_ref()
            .and_then(|x| x.status.as_deref()),
        Some("1")
    );
}

#[test]
fn batch_get_maps_status_codes_to_safe_errors() {
    let mut unauthorized = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(401, "{}")),
    );
    assert_eq!(
        unauthorized.batch_get_okrs(sample_request()),
        Err(FeishuOkrReadError::Unauthorized)
    );

    let mut forbidden = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(403, "{}")),
    );
    assert_eq!(
        forbidden.batch_get_okrs(sample_request()),
        Err(FeishuOkrReadError::Forbidden)
    );

    let mut server_error = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(503, "{}")),
    );
    assert_eq!(
        server_error.batch_get_okrs(sample_request()),
        Err(FeishuOkrReadError::UpstreamTransient)
    );
}

#[test]
fn batch_get_fail_closed_for_oversized_and_invalid_json() {
    let mut oversized = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_error(HttpClientFailure::OversizedResponse {
            max_response_bytes: 32,
        }),
    );
    assert_eq!(
        oversized.batch_get_okrs(sample_request()),
        Err(FeishuOkrReadError::OversizedResponse)
    );

    let mut invalid_json = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(200, "{not-json")),
    );
    assert_eq!(
        invalid_json.batch_get_okrs(sample_request()),
        Err(FeishuOkrReadError::InvalidJson)
    );
}

#[test]
fn token_is_redacted_in_okr_request_debug_and_errors() {
    let request = sample_request();
    let debug = format!("{request:?}");
    assert!(!debug.contains("u-very-secret-token"));
    assert!(debug.contains("[REDACTED]"));

    let error_debug = format!("{:?}", FeishuOkrReadError::Unauthorized);
    let error_display = FeishuOkrReadError::Unauthorized.to_string();
    assert!(!error_debug.contains("u-very-secret-token"));
    assert!(!error_display.contains("u-very-secret-token"));
}

#[test]
fn batch_get_request_uses_get_and_query_parameters() {
    let mut client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({"code":0,"data":{"okr_list":[]}}).to_string(),
        )),
    );
    client.batch_get_okrs(sample_request()).expect("success");
    let sent = client
        .http_client()
        .request
        .as_ref()
        .expect("captured request");
    assert_eq!(sent.method, "GET");
    assert!(sent.url.contains("user_id_type=open_id"));
    assert!(sent.url.contains("okr_ids=okr_1"));
    assert!(sent.url.contains("okr_ids=okr_2"));
    assert!(sent.url.contains("lang=zh_cn"));
    assert_eq!(sent.body, json!({}));
    let debug = format!("{sent:?}");
    assert!(!debug.contains("u-very-secret-token"));
    assert!(!debug.contains("Bearer u-very-secret-token"));
    assert!(debug.contains("[REDACTED]"));
}

#[test]
fn batch_get_rejects_more_than_ten_okr_ids() {
    let mut client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({"code":0,"data":{"okr_list":[]}}).to_string(),
        )),
    );
    let mut request = sample_request();
    request.okr_ids = (0..11).map(|i| format!("okr_{i}")).collect();
    assert_eq!(
        client.batch_get_okrs(request),
        Err(FeishuOkrReadError::InvalidRequest)
    );
}

#[test]
fn current_cycles_list_request_uses_get_query_parameters_and_redacts_debug() {
    let mut client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({"code":0,"data":{"items":[],"has_more":false}}).to_string(),
        )),
    );

    let request = sample_cycle_list_request();
    let request_debug = format!("{request:?}");
    assert!(!request_debug.contains("u-very-secret-token"));
    assert!(request_debug.contains("[REDACTED]"));

    client.list_cycles(request).expect("success");
    let sent = client
        .http_client()
        .request
        .as_ref()
        .expect("captured request");
    assert_eq!(sent.method, "GET");
    assert!(sent
        .url
        .starts_with("https://open.feishu.cn/open-apis/okr/v2/cycles?"));
    assert!(sent.url.contains("user_id_type=open_id"));
    assert!(sent.url.contains("user_id=ou_user_1"));
    assert!(sent.url.contains("page_size=100"));
    assert!(sent.url.contains("page_token=next%20token%2F1"));
    assert!(sent.url.contains("lang=zh_cn"));
    assert_eq!(sent.body, json!({}));
    assert!(sent
        .headers
        .iter()
        .any(|(name, value)| { name == "Authorization" && value == "Bearer u-very-secret-token" }));

    let debug = format!("{sent:?}");
    assert!(!debug.contains("u-very-secret-token"));
    assert!(!debug.contains("Bearer u-very-secret-token"));
    assert!(debug.contains("[REDACTED]"));
}

#[test]
fn current_objectives_and_key_results_requests_path_encode_ids() {
    let mut objectives_client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({"code":0,"data":{"items":[],"has_more":false}}).to_string(),
        )),
    );
    objectives_client
        .list_cycle_objectives(sample_cycle_objectives_request())
        .expect("success");
    let objectives_request = objectives_client
        .http_client()
        .request
        .as_ref()
        .expect("captured objectives request");
    assert!(objectives_request
        .url
        .contains("/open-apis/okr/v2/cycles/cycle%202026%2F05/objectives?"));
    assert!(objectives_request.url.contains("user_id_type=open_id"));
    assert!(objectives_request
        .url
        .contains("page_token=objective%20token%2F1"));

    let mut key_results_client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({"code":0,"data":{"items":[],"has_more":false}}).to_string(),
        )),
    );
    key_results_client
        .list_objective_key_results(sample_objective_key_results_request())
        .expect("success");
    let key_results_request = key_results_client
        .http_client()
        .request
        .as_ref()
        .expect("captured key results request");
    assert!(key_results_request
        .url
        .contains("/open-apis/okr/v2/objectives/obj%2F1%3Fx/key_results?"));
    assert!(key_results_request.url.contains("user_id_type=open_id"));
    assert!(key_results_request
        .url
        .contains("page_token=kr%20token%2F1"));
}

#[test]
fn current_list_responses_parse_to_safe_domain_pages() {
    let mut cycles_client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({
                "code": 0,
                "data": {
                    "items": [{
                        "cycle_id": "cycle_2026_05",
                        "name": "2026-05 to 2026-07",
                        "start_time": 1777564800000_i64,
                        "end_time": "1785427200000",
                        "status": 1,
                        "raw_field": "does not enter domain page"
                    }],
                    "page_token": "next-cycle",
                    "has_more": true
                }
            })
            .to_string(),
        )),
    );
    let cycles = cycles_client
        .list_cycles(sample_cycle_list_request())
        .expect("cycles");
    let cycle_page = OkrReadCyclesPage::from_cycle_list_data(&cycles.data.expect("cycle data"));
    assert_eq!(cycle_page.cycles.len(), 1);
    assert_eq!(
        cycle_page.cycles[0].cycle_id.as_deref(),
        Some("cycle_2026_05")
    );
    assert_eq!(cycle_page.cycles[0].status.as_deref(), Some("1"));
    assert_eq!(cycle_page.next_page_token.as_deref(), Some("next-cycle"));
    assert!(cycle_page.has_more);

    let mut objectives_client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({
                "code": 0,
                "data": {
                    "objectives": [{
                        "objective_id": "obj_1",
                        "content": {"text": "Grow objective"},
                        "notes": "{\"text\":\"private note text\"}",
                        "progress_rate": {"percent": "50", "status": 2},
                        "key_results": [{
                            "kr_id": "kr_inline",
                            "content": [{"text": "Inline KR"}]
                        }]
                    }],
                    "next_page_token": "next-objective",
                    "has_more": false
                }
            })
            .to_string(),
        )),
    );
    let objectives = objectives_client
        .list_cycle_objectives(sample_cycle_objectives_request())
        .expect("objectives");
    let objective_data = objectives.data.expect("objective data");
    assert_eq!(
        objective_data.items[0].notes_text().as_deref(),
        Some("private note text")
    );
    let objective_page =
        OkrReadObjectivesPage::from_cycle_objectives_list_data("cycle_2026_05", &objective_data);
    assert_eq!(objective_page.objectives.len(), 1);
    assert_eq!(
        objective_page.objectives[0].content.as_deref(),
        Some("Grow objective")
    );
    assert_eq!(objective_page.objectives[0].status.as_deref(), Some("2"));
    assert_eq!(
        objective_page.next_page_token.as_deref(),
        Some("next-objective")
    );
    assert!(!objective_page.has_more);
    assert_eq!(
        objective_page.objectives[0].krs[0].content.as_deref(),
        Some("Inline KR")
    );

    let mut key_results_client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({
                "code": 0,
                "data": {
                    "key_results": [{
                        "id": "kr_1",
                        "content": "{\"text\":\"Ship current OKR read\"}",
                        "notes": [{"text":"KR note"}],
                        "progress_rate": {"percent": 80, "status": "normal"}
                    }]
                }
            })
            .to_string(),
        )),
    );
    let key_results = key_results_client
        .list_objective_key_results(sample_objective_key_results_request())
        .expect("key results");
    let key_result_data = key_results.data.expect("key result data");
    assert_eq!(
        key_result_data.items[0].notes_text().as_deref(),
        Some("KR note")
    );
    let key_result_page =
        OkrReadKeyResultsPage::from_objective_key_results_list_data("obj_1", &key_result_data);
    assert_eq!(key_result_page.krs.len(), 1);
    assert_eq!(key_result_page.krs[0].kr_id.as_deref(), Some("kr_1"));
    assert_eq!(
        key_result_page.krs[0].content.as_deref(),
        Some("Ship current OKR read")
    );
    assert_eq!(key_result_page.krs[0].progress.as_deref(), Some("80"));
}

#[test]
fn current_list_apis_map_401_403_invalid_json_and_invalid_request() {
    let mut unauthorized = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(401, "{}")),
    );
    assert_eq!(
        unauthorized.list_cycles(sample_cycle_list_request()),
        Err(FeishuOkrReadError::Unauthorized)
    );

    let mut forbidden = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(403, "{}")),
    );
    assert_eq!(
        forbidden.list_cycle_objectives(sample_cycle_objectives_request()),
        Err(FeishuOkrReadError::Forbidden)
    );

    let mut invalid_json = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(200, "{not-json")),
    );
    assert_eq!(
        invalid_json.list_objective_key_results(sample_objective_key_results_request()),
        Err(FeishuOkrReadError::InvalidJson)
    );

    let mut invalid_page_size = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({"code":0,"data":{"items":[]}}).to_string(),
        )),
    );
    let mut request = sample_cycle_list_request();
    request.page_size = Some(101);
    assert_eq!(
        invalid_page_size.list_cycles(request),
        Err(FeishuOkrReadError::InvalidRequest)
    );

    let mut empty_path_id = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({"code":0,"data":{"items":[]}}).to_string(),
        )),
    );
    let mut request = sample_cycle_objectives_request();
    request.cycle_id = " ".to_string();
    assert_eq!(
        empty_path_id.list_cycle_objectives(request),
        Err(FeishuOkrReadError::InvalidRequest)
    );
}

#[test]
fn async_batch_get_success_response_parses() {
    let response = HttpResponse::new(
        200,
        json!({
            "code": 0,
            "data": { "okr_list": [{"id":"okr_1"}] }
        })
        .to_string(),
    );
    let mut client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        AsyncFakeHttpClient { response },
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let parsed = runtime
        .block_on(client.batch_get_okrs(sample_request()))
        .expect("success");
    assert_eq!(parsed.code, 0);
}

#[test]
fn async_current_cycles_list_success_response_parses() {
    let response = HttpResponse::new(
        200,
        json!({
            "code": 0,
            "data": { "items": [{"cycle_id":"cycle_async"}], "has_more": false }
        })
        .to_string(),
    );
    let mut client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        AsyncFakeHttpClient { response },
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let parsed = runtime
        .block_on(client.list_cycles(sample_cycle_list_request()))
        .expect("success");
    assert_eq!(
        parsed
            .data
            .expect("data")
            .items
            .first()
            .and_then(|cycle| cycle.id.as_deref()),
        Some("cycle_async")
    );
}

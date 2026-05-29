use serde_json::json;

use super::helpers::{
    sample_cycle_list_request, sample_cycle_objectives_request,
    sample_objective_key_results_request, AsyncFakeHttpClient, FakeHttpClient,
};
use crate::config::FeishuOpenApiConfig;
use crate::oauth::HttpResponse;
use crate::okr::{
    AsyncFeishuOkrRead, FeishuOkrReadClient, FeishuOkrReadError, OkrReadCyclesPage,
    OkrReadKeyResultsPage, OkrReadObjectivesPage,
};

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

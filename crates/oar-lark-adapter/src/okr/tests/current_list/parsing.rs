use serde_json::json;

use super::{
    sample_cycle_list_request, sample_cycle_objectives_request,
    sample_objective_key_results_request, FakeHttpClient,
};
use crate::config::FeishuOpenApiConfig;
use crate::oauth::HttpResponse;
use crate::okr::{
    FeishuOkrReadClient, OkrReadCyclesPage, OkrReadKeyResultsPage, OkrReadObjectivesPage,
};

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

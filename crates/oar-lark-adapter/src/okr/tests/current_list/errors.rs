use serde_json::json;

use super::{
    sample_cycle_list_request, sample_cycle_objectives_request,
    sample_objective_key_results_request, FakeHttpClient,
};
use crate::config::FeishuOpenApiConfig;
use crate::oauth::HttpResponse;
use crate::okr::{FeishuOkrReadClient, FeishuOkrReadError};

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

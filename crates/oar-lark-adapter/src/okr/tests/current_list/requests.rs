use serde_json::json;

use super::{
    sample_cycle_list_request, sample_cycle_objectives_request,
    sample_objective_key_results_request, FakeHttpClient,
};
use crate::config::FeishuOpenApiConfig;
use crate::oauth::HttpResponse;
use crate::okr::FeishuOkrReadClient;

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

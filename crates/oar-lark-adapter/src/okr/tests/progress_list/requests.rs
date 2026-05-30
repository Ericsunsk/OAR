use serde_json::json;

use super::{sample_progress_list_request, FakeHttpClient};
use crate::config::FeishuOpenApiConfig;
use crate::oauth::HttpResponse;
use crate::okr::{
    FeishuOkrProgressListTarget, FeishuOkrReadClient, FeishuOkrReadError, OkrDepartmentIdType,
    OkrUserIdType,
};

#[test]
fn progress_list_request_uses_v2_target_paths_defaults_and_redacts() {
    let mut objective_client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({"code":0,"data":{"progress_list":[],"has_more":false}}).to_string(),
        )),
    );
    let request = sample_progress_list_request(FeishuOkrProgressListTarget::Objective(
        "obj/1?x".to_string(),
    ));
    let request_debug = format!("{request:?}");
    assert!(!request_debug.contains("u-very-secret-token"));
    assert!(request_debug.contains("[REDACTED]"));

    objective_client.list_progress(request).expect("success");
    let objective_sent = objective_client
        .http_client()
        .request
        .as_ref()
        .expect("captured objective progress request");
    assert_eq!(objective_sent.method, "GET");
    assert!(objective_sent
        .url
        .contains("/open-apis/okr/v2/objectives/obj%2F1%3Fx/progresses?"));
    assert!(objective_sent.url.contains("user_id_type=open_id"));
    assert!(objective_sent
        .url
        .contains("department_id_type=open_department_id"));
    assert!(objective_sent.url.contains("page_size=100"));
    assert!(objective_sent
        .url
        .contains("page_token=progress%20token%2F1"));
    assert!(!objective_sent.url.contains("progress_records"));
    assert_eq!(objective_sent.body, json!({}));
    assert!(objective_sent
        .headers
        .iter()
        .any(|(name, value)| { name == "Authorization" && value == "Bearer u-very-secret-token" }));
    let objective_debug = format!("{objective_sent:?}");
    assert!(!objective_debug.contains("u-very-secret-token"));
    assert!(!objective_debug.contains("Bearer u-very-secret-token"));
    assert!(objective_debug.contains("[REDACTED]"));

    let mut key_result_client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({"code":0,"data":{"progress_list":[],"has_more":false}}).to_string(),
        )),
    );
    let mut request =
        sample_progress_list_request(FeishuOkrProgressListTarget::KeyResult("kr 1/2".to_string()));
    request.user_id_type = OkrUserIdType::UnionId;
    request.department_id_type = OkrDepartmentIdType::DepartmentId;
    request.page_size = Some(50);
    request.page_token = None;

    key_result_client.list_progress(request).expect("success");
    let key_result_sent = key_result_client
        .http_client()
        .request
        .as_ref()
        .expect("captured key result progress request");
    assert!(key_result_sent
        .url
        .contains("/open-apis/okr/v2/key_results/kr%201%2F2/progresses?"));
    assert!(key_result_sent.url.contains("user_id_type=union_id"));
    assert!(key_result_sent
        .url
        .contains("department_id_type=department_id"));
    assert!(key_result_sent.url.contains("page_size=50"));
    assert!(!key_result_sent.url.contains("page_token="));
}

#[test]
fn progress_list_invalid_target_page_size_and_page_token_fail_closed() {
    let success = HttpResponse::new(
        200,
        json!({"code":0,"data":{"progress_list":[]}}).to_string(),
    );

    let mut empty_target_client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(success.clone()),
    );
    let request =
        sample_progress_list_request(FeishuOkrProgressListTarget::Objective(" ".to_string()));
    assert_eq!(
        empty_target_client.list_progress(request),
        Err(FeishuOkrReadError::InvalidRequest)
    );
    assert!(empty_target_client.http_client().request.is_none());

    let mut zero_page_size_client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(success.clone()),
    );
    let mut request =
        sample_progress_list_request(FeishuOkrProgressListTarget::KeyResult("kr_1".to_string()));
    request.page_size = Some(0);
    assert_eq!(
        zero_page_size_client.list_progress(request),
        Err(FeishuOkrReadError::InvalidRequest)
    );
    assert!(zero_page_size_client.http_client().request.is_none());

    let mut large_page_size_client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(success.clone()),
    );
    let mut request =
        sample_progress_list_request(FeishuOkrProgressListTarget::KeyResult("kr_1".to_string()));
    request.page_size = Some(101);
    assert_eq!(
        large_page_size_client.list_progress(request),
        Err(FeishuOkrReadError::InvalidRequest)
    );
    assert!(large_page_size_client.http_client().request.is_none());

    let mut blank_page_token_client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(success),
    );
    let mut request =
        sample_progress_list_request(FeishuOkrProgressListTarget::Objective("obj_1".to_string()));
    request.page_token = Some(" ".to_string());
    assert_eq!(
        blank_page_token_client.list_progress(request),
        Err(FeishuOkrReadError::InvalidRequest)
    );
    assert!(blank_page_token_client.http_client().request.is_none());
}

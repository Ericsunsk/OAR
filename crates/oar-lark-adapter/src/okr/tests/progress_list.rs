use serde_json::json;

use super::helpers::{sample_progress_list_request, AsyncFakeHttpClient, FakeHttpClient};
use crate::config::FeishuOpenApiConfig;
use crate::oauth::HttpResponse;
use crate::okr::{
    AsyncFeishuOkrRead, FeishuOkrProgressListTarget, FeishuOkrReadClient, FeishuOkrReadError,
    OkrDepartmentIdType, OkrReadProgressPage, OkrUserIdType,
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

#[test]
fn progress_list_success_parses_progress_list_and_safe_page_is_sanitized() {
    let response = HttpResponse::new(
        200,
        json!({
            "code": 0,
            "msg": "ok",
            "data": {
                "progress_list": [
                    {
                        "progress_id": "pr_1",
                        "modify_time": 1780000000000_i64,
                        "content": {"text": "private progress body"},
                        "person": {"name": "Alice Example"},
                        "image_list": [{"file_token": "img_secret"}],
                        "progress_rate": {"percent": 75.5, "status": "normal"}
                    },
                    {
                        "id": 12345,
                        "modify_time": "2026-05-20T10:00:00Z",
                        "progress_rate": {"percent": "80", "status": 2}
                    }
                ],
                "next_page_token": "next-progress",
                "has_more": true
            }
        })
        .to_string(),
    );
    let mut client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(response),
    );
    let parsed = client
        .list_progress(sample_progress_list_request(
            FeishuOkrProgressListTarget::Objective("obj_1".to_string()),
        ))
        .expect("success");
    assert_eq!(parsed.code, 0);
    let data = parsed.data.expect("data");
    assert_eq!(data.progress_list.len(), 2);
    assert!(data.progress_list[0].extra.contains_key("content"));
    assert!(data.progress_list[0].extra.contains_key("person"));
    assert_eq!(data.progress_list[0].progress_id.as_deref(), Some("pr_1"));
    assert_eq!(data.progress_list[1].progress_id.as_deref(), Some("12345"));

    let page = OkrReadProgressPage::from_progress_list_data(&data);
    assert_eq!(page.progress_records.len(), 2);
    assert_eq!(page.next_page_token.as_deref(), Some("next-progress"));
    assert!(page.has_more);
    assert_eq!(page.progress_records[0].id.as_deref(), Some("pr_1"));
    assert_eq!(
        page.progress_records[0].modify_time.as_deref(),
        Some("1780000000000")
    );
    assert_eq!(page.progress_records[0].percent.as_deref(), Some("75.5"));
    assert_eq!(page.progress_records[0].status.as_deref(), Some("normal"));
    assert_eq!(page.progress_records[1].id.as_deref(), Some("12345"));
    assert_eq!(page.progress_records[1].status.as_deref(), Some("2"));

    let safe_json = serde_json::to_string(&page).expect("safe json");
    assert!(!safe_json.contains("private progress body"));
    assert!(!safe_json.contains("Alice Example"));
    assert!(!safe_json.contains("img_secret"));
    assert!(!safe_json.contains("content"));
    assert!(!safe_json.contains("person"));
    assert!(!safe_json.contains("image_list"));
}

#[test]
fn progress_list_maps_http_status_api_code_and_json_errors() {
    let mut unauthorized = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(401, "{}")),
    );
    assert_eq!(
        unauthorized.list_progress(sample_progress_list_request(
            FeishuOkrProgressListTarget::Objective("obj_1".to_string())
        )),
        Err(FeishuOkrReadError::Unauthorized)
    );

    let mut forbidden = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(403, "{}")),
    );
    assert_eq!(
        forbidden.list_progress(sample_progress_list_request(
            FeishuOkrProgressListTarget::KeyResult("kr_1".to_string())
        )),
        Err(FeishuOkrReadError::Forbidden)
    );

    let mut upstream_client = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(400, "{}")),
    );
    assert_eq!(
        upstream_client.list_progress(sample_progress_list_request(
            FeishuOkrProgressListTarget::Objective("obj_1".to_string())
        )),
        Err(FeishuOkrReadError::UpstreamClient)
    );

    let mut api_unauthorized = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({"code":99991663,"msg":"token invalid"}).to_string(),
        )),
    );
    assert_eq!(
        api_unauthorized.list_progress(sample_progress_list_request(
            FeishuOkrProgressListTarget::Objective("obj_1".to_string())
        )),
        Err(FeishuOkrReadError::Unauthorized)
    );

    let mut api_forbidden = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({"code":403,"msg":"forbidden"}).to_string(),
        )),
    );
    assert_eq!(
        api_forbidden.list_progress(sample_progress_list_request(
            FeishuOkrProgressListTarget::KeyResult("kr_1".to_string())
        )),
        Err(FeishuOkrReadError::Forbidden)
    );

    let mut api_no_permission = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(
            200,
            json!({"code":1001002,"msg":"no permission"}).to_string(),
        )),
    );
    assert_eq!(
        api_no_permission.list_progress(sample_progress_list_request(
            FeishuOkrProgressListTarget::Objective("obj_1".to_string())
        )),
        Err(FeishuOkrReadError::Forbidden)
    );

    let mut invalid_json = FeishuOkrReadClient::new(
        FeishuOpenApiConfig::default(),
        FakeHttpClient::from_response(HttpResponse::new(200, "{not-json")),
    );
    assert_eq!(
        invalid_json.list_progress(sample_progress_list_request(
            FeishuOkrProgressListTarget::Objective("obj_1".to_string())
        )),
        Err(FeishuOkrReadError::InvalidJson)
    );
}

#[test]
fn async_progress_list_success_response_parses() {
    let response = HttpResponse::new(
        200,
        json!({
            "code": 0,
            "data": {
                "progress_list": [{
                    "progress_id": "pr_async",
                    "modify_time": "2026-05-29T10:00:00Z",
                    "progress_rate": {"percent": 90, "status": "done"}
                }],
                "page_token": "next-async",
                "has_more": true
            }
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
        .block_on(client.list_progress(sample_progress_list_request(
            FeishuOkrProgressListTarget::KeyResult("kr_async".to_string()),
        )))
        .expect("success");
    let data = parsed.data.expect("data");
    assert_eq!(
        data.progress_list
            .first()
            .and_then(|progress| progress.progress_id.as_deref()),
        Some("pr_async")
    );
    let page = OkrReadProgressPage::from_progress_list_data(&data);
    assert_eq!(page.next_page_token.as_deref(), Some("next-async"));
    assert_eq!(page.progress_records[0].percent.as_deref(), Some("90"));
    assert_eq!(page.progress_records[0].status.as_deref(), Some("done"));
    assert!(page.has_more);
}

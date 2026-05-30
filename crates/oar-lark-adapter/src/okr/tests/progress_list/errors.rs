use serde_json::json;

use super::{sample_progress_list_request, FakeHttpClient};
use crate::config::FeishuOpenApiConfig;
use crate::oauth::HttpResponse;
use crate::okr::{FeishuOkrProgressListTarget, FeishuOkrReadClient, FeishuOkrReadError};

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

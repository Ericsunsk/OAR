use serde_json::json;

use super::{sample_request, FakeHttpClient};
use crate::config::FeishuOpenApiConfig;
use crate::oauth::HttpResponse;
use crate::okr::{FeishuOkrReadClient, FeishuOkrReadError};

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
    assert_eq!(
        sent.url,
        concat!(
            "https://open.feishu.cn/open-apis/okr/v1/okrs/batch_get?",
            "user_id_type=open_id",
            "&okr_ids=okr_1",
            "&okr_ids=okr_2",
            "&lang=zh_cn"
        )
    );
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

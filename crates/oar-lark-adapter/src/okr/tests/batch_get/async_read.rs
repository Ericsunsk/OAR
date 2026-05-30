use serde_json::json;

use super::{sample_request, AsyncFakeHttpClient};
use crate::config::FeishuOpenApiConfig;
use crate::oauth::HttpResponse;
use crate::okr::{AsyncFeishuOkrRead, FeishuOkrReadClient};

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

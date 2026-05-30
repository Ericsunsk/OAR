use serde_json::json;

use super::{sample_cycle_list_request, AsyncFakeHttpClient};
use crate::config::FeishuOpenApiConfig;
use crate::oauth::HttpResponse;
use crate::okr::{AsyncFeishuOkrRead, FeishuOkrReadClient};

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

use serde_json::json;

use super::{sample_progress_list_request, AsyncFakeHttpClient};
use crate::config::FeishuOpenApiConfig;
use crate::oauth::HttpResponse;
use crate::okr::{
    AsyncFeishuOkrRead, FeishuOkrProgressListTarget, FeishuOkrReadClient, OkrReadProgressPage,
};

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

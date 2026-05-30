use std::cell::RefCell;
use std::rc::Rc;

use super::{
    sample_request, CountingHttpClient, FailingMaterialProvider, FakeEncryptor,
    FeishuAuthRefreshClient, FeishuAuthRefreshFailure, FeishuAuthRefreshResponse,
    FeishuAuthRefreshSafeClient, FeishuOAuthTransport, FeishuOpenApiConfig,
};

#[test]
fn material_provider_failure_maps_to_transient_and_skips_http_in_safe_client() {
    let sent_requests = Rc::new(RefCell::new(0usize));
    let transport = FeishuOAuthTransport::new(
        FeishuOpenApiConfig::default(),
        FailingMaterialProvider,
        FakeEncryptor,
        CountingHttpClient::new(sent_requests.clone()),
    );
    let mut client = FeishuAuthRefreshSafeClient::new(transport);

    let response = client
        .refresh(&sample_request())
        .expect("safe client should fail closed to transient");

    assert_eq!(
        response,
        FeishuAuthRefreshResponse::Failure(FeishuAuthRefreshFailure::Transient {
            safe_error: "temporarily unavailable".to_string()
        })
    );
    assert_eq!(*sent_requests.borrow(), 0, "http must not be called");
}

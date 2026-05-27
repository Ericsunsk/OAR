use std::cell::RefCell;
use std::rc::Rc;

use oar_core::domain::token_refresh::service::AsyncAuthRefreshAdapter;
use oar_core::domain::token_refresh::types::RefreshOutcome;
use oar_core::lark::auth::client::LarkAuthRefreshSafeClient;
use oar_core::lark::auth::types::LarkAuthRefreshResponse;

use super::helpers::{
    assert_no_secret, contains_subslice, runtime, sample_request, snapshot, stored_provider,
    success_body, AsyncFailingMaterialProvider, AsyncFakeHttpClient, CountingAsyncHttpClient,
    FakeEncryptor, FixedClock, ACCESS_TOKEN, REFRESH_TOKEN,
};
use crate::crypto::AesGcmGrantEncryptor;
use crate::oauth::{FeishuOAuthTransport, HttpResponse};
use crate::FeishuOpenApiConfig;

#[test]
fn async_adapter_material_provider_failure_maps_to_transient_and_skips_http() {
    let sent_requests = Rc::new(RefCell::new(0usize));
    let transport = FeishuOAuthTransport::new(
        FeishuOpenApiConfig::default(),
        AsyncFailingMaterialProvider,
        FakeEncryptor,
        CountingAsyncHttpClient::new(sent_requests.clone()),
    );
    let safe_client = LarkAuthRefreshSafeClient::new(transport);
    let mut adapter = oar_core::lark::auth::adapter::LarkAuthRefreshAdapter::new(safe_client);

    let outcome = runtime().block_on(adapter.refresh(&snapshot()));

    assert_eq!(
        outcome,
        RefreshOutcome::TransientFailure {
            safe_error: "temporarily unavailable".to_string(),
        }
    );
    assert_eq!(*sent_requests.borrow(), 0, "http must not be called");
}

#[test]
fn stored_blob_to_async_core_adapter_success_rotates_encrypted_material() {
    let key = [5; 32];
    let provider = stored_provider(key, "urt-stored-renewal");
    let transport = FeishuOAuthTransport::new(
        FeishuOpenApiConfig::default(),
        provider,
        AesGcmGrantEncryptor::with_clock(
            "key-1",
            key,
            FixedClock {
                now_ms: 1_779_465_600_000,
            },
        ),
        AsyncFakeHttpClient::from_response(HttpResponse::new(200, success_body())),
    );
    let safe_client = LarkAuthRefreshSafeClient::new(transport);
    let mut adapter = oar_core::lark::auth::adapter::LarkAuthRefreshAdapter::new(safe_client);

    let outcome = runtime().block_on(adapter.refresh(&snapshot()));

    match outcome {
        RefreshOutcome::Success {
            rotated_material,
            key_id,
            new_fingerprint,
            refreshed_at,
            expires_at,
        } => {
            assert!(!rotated_material.encrypted_primary.is_empty());
            assert!(!rotated_material.encrypted_renewal.is_empty());
            assert_eq!(key_id, "key-1");
            assert!(!new_fingerprint.is_empty());
            assert_ne!(new_fingerprint, "fp-current");
            assert_eq!(
                refreshed_at,
                std::time::UNIX_EPOCH + std::time::Duration::from_millis(1_779_465_600_000)
            );
            assert_eq!(
                expires_at,
                Some(std::time::UNIX_EPOCH + std::time::Duration::from_millis(1_779_472_800_000))
            );
            assert!(!contains_subslice(
                &rotated_material.encrypted_primary,
                ACCESS_TOKEN.as_bytes()
            ));
            assert!(!contains_subslice(
                &rotated_material.encrypted_renewal,
                REFRESH_TOKEN.as_bytes()
            ));
        }
        other => panic!("expected success, got {other:?}"),
    }

    let debug = format!("{adapter:?}");
    assert_no_secret(&debug);
    assert!(!debug.contains("urt-stored-renewal"));
    assert!(!debug.contains("fp-current"));
}

#[test]
fn async_safe_client_transient_failure_is_safe() {
    let transport = FeishuOAuthTransport::new(
        FeishuOpenApiConfig::default(),
        AsyncFailingMaterialProvider,
        FakeEncryptor,
        CountingAsyncHttpClient::new(Rc::new(RefCell::new(0))),
    );
    let mut safe_client = LarkAuthRefreshSafeClient::new(transport);

    let response = runtime().block_on(async { safe_client.refresh_async(&sample_request()).await });

    match response {
        Ok(LarkAuthRefreshResponse::Failure(failure)) => {
            let rendered = format!("{failure:?}");
            assert_no_secret(&rendered);
        }
        other => panic!("expected async failure response, got {other:?}"),
    }
}

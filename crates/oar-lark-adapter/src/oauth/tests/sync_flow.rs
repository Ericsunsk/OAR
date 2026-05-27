use std::cell::RefCell;
use std::rc::Rc;

use oar_core::lark::auth::adapter::LarkAuthRefreshClient;
use oar_core::lark::auth::client::{LarkAuthRefreshSafeClient, LarkAuthRefreshTransport};
use oar_core::lark::auth::types::{LarkAuthRefreshFailure, LarkAuthRefreshResponse};
use serde_json::json;

use super::helpers::{
    assert_no_secret, error_body, sample_envelope, sample_material, sample_request,
    sample_transport, success_body, transport_with_http_error, CountingHttpClient,
    FailingMaterialProvider, FakeEncryptor, ACCESS_TOKEN, CLIENT_SECRET, REFRESH_TOKEN,
};
use crate::oauth::{FeishuOAuthTransport, HttpClientFailure, HttpRequest, HttpResponse};
use crate::redaction::SecretString;
use crate::FeishuOpenApiConfig;

#[test]
fn success_response_is_encrypted_before_core_sees_it() {
    let transport = sample_transport(HttpResponse::new(200, success_body()));
    let mut client = LarkAuthRefreshSafeClient::new(transport);

    let response = client
        .refresh(&sample_request())
        .expect("safe envelope should parse");

    match response {
        LarkAuthRefreshResponse::Success(success) => {
            assert_eq!(success.encrypted_primary, vec![11, 12, 13]);
            assert_eq!(success.encrypted_renewal, vec![21, 22, 23]);
            assert_eq!(success.key_id, "kms-test");
            assert_eq!(success.new_fingerprint, "fp-rotated");
            let debug = format!("{success:?}");
            assert_no_secret(&debug);
        }
        other => panic!("expected success, got {other:?}"),
    }

    let envelope = sample_envelope();
    let safe_value = serde_json::json!({
        "outcome": "success",
        "encrypted_primary": envelope.encrypted_primary,
        "encrypted_renewal": envelope.encrypted_renewal,
        "key_id": envelope.key_id,
        "new_fingerprint": envelope.new_fingerprint,
        "refreshed_at_ms": envelope.refreshed_at_ms,
        "expires_at_ms": envelope.expires_at_ms,
    });
    let safe_json = serde_json::to_string(&safe_value).expect("safe value serializes");
    assert!(!safe_json.contains(ACCESS_TOKEN));
    assert!(!safe_json.contains(REFRESH_TOKEN));
    assert!(!safe_json.contains("access_token"));
    assert!(!safe_json.contains("refresh_token"));
}

#[test]
fn feishu_invalid_grant_codes_map_to_reauth_required() {
    for code in [20037, 20064, 20073] {
        let transport = sample_transport(HttpResponse::new(400, error_body(code)));
        let mut client = LarkAuthRefreshSafeClient::new(transport);

        let response = client
            .refresh(&sample_request())
            .expect("safe failure envelope should parse");

        assert_eq!(
            response,
            LarkAuthRefreshResponse::Failure(LarkAuthRefreshFailure::ReauthRequired {
                safe_error: "invalid_grant".to_string()
            })
        );
    }
}

#[test]
fn feishu_refresh_disabled_maps_to_config_required() {
    let transport = sample_transport(HttpResponse::new(400, error_body(20074)));
    let mut client = LarkAuthRefreshSafeClient::new(transport);

    let response = client
        .refresh(&sample_request())
        .expect("safe failure envelope should parse");

    assert_eq!(
        response,
        LarkAuthRefreshResponse::Failure(LarkAuthRefreshFailure::ConfigRequired {
            safe_error: "refresh_config_required".to_string()
        })
    );
}

#[test]
fn feishu_transient_codes_http_5xx_transport_and_oversized_map_to_transient() {
    let cases = [
        sample_transport(HttpResponse::new(500, "not json")),
        sample_transport(HttpResponse::new(500, error_body(20050))),
        sample_transport(HttpResponse::new(503, error_body(20072))),
        transport_with_http_error(HttpClientFailure::Transport),
        transport_with_http_error(HttpClientFailure::OversizedResponse {
            max_response_bytes: 16,
        }),
    ];

    for transport in cases {
        let mut client = LarkAuthRefreshSafeClient::new(transport);
        let response = client
            .refresh(&sample_request())
            .expect("transient safe envelope should parse");
        assert_eq!(
            response,
            LarkAuthRefreshResponse::Failure(LarkAuthRefreshFailure::Transient {
                safe_error: "temporarily unavailable".to_string()
            })
        );
    }
}

#[test]
fn material_provider_failure_maps_to_transient_and_skips_http_in_safe_client() {
    let sent_requests = Rc::new(RefCell::new(0usize));
    let transport = FeishuOAuthTransport::new(
        FeishuOpenApiConfig::default(),
        FailingMaterialProvider,
        FakeEncryptor,
        CountingHttpClient::new(sent_requests.clone()),
    );
    let mut client = LarkAuthRefreshSafeClient::new(transport);

    let response = client
        .refresh(&sample_request())
        .expect("safe client should fail closed to transient");

    assert_eq!(
        response,
        LarkAuthRefreshResponse::Failure(LarkAuthRefreshFailure::Transient {
            safe_error: "temporarily unavailable".to_string()
        })
    );
    assert_eq!(*sent_requests.borrow(), 0, "http must not be called");
}

#[test]
fn request_shape_matches_feishu_refresh_openapi() {
    let mut transport = sample_transport(HttpResponse::new(200, success_body()));

    transport
        .execute(&sample_request())
        .expect("transport should return safe envelope");

    let sent = &transport.http_client().requests[0];
    assert_eq!(sent.method, "POST");
    assert_eq!(
        sent.url,
        "https://open.feishu.cn/open-apis/authen/v2/oauth/token"
    );
    assert_eq!(
        sent.headers,
        vec![
            (
                "Content-Type".to_string(),
                "application/json; charset=utf-8".to_string()
            ),
            ("Accept".to_string(), "application/json".to_string()),
            (
                "User-Agent".to_string(),
                format!("oar-lark-adapter/{}", env!("CARGO_PKG_VERSION"))
            )
        ]
    );
    assert_eq!(sent.body["grant_type"], "refresh_token");
    assert_eq!(sent.body["client_id"], "cli_test");
    assert_eq!(sent.body["client_secret"], CLIENT_SECRET);
    assert_eq!(sent.body["refresh_token"], REFRESH_TOKEN);
    assert_eq!(sent.body["scope"], "offline_access auth:user.id:read");

    let debug = format!("{sent:?}");
    assert_no_secret(&debug);
}

#[test]
fn reqwest_client_accepts_timeout_config() {
    let client = crate::oauth::ReqwestBlockingHttpClient::with_config(&FeishuOpenApiConfig {
        base_url: "https://open.feishu.cn".to_string(),
        max_response_bytes: 1024,
        request_timeout_ms: 1_500,
        connect_timeout_ms: 500,
    })
    .expect("timeout config should build reqwest client");

    let debug = format!("{client:?}");
    assert_no_secret(&debug);
}

#[test]
fn debug_and_display_redact_secrets_and_raw_bodies() {
    let material = sample_material();
    let input = crate::oauth::FeishuGrantEncryptionInput {
        grant_id: "grant-1".to_string(),
        tenant_id: "tenant-1".to_string(),
        expected_fingerprint: "fp-secret".to_string(),
        access_token: SecretString::new(ACCESS_TOKEN),
        refresh_token: SecretString::new(REFRESH_TOKEN),
        expires_in_seconds: 7200,
        refresh_token_expires_in_seconds: Some(604800),
        token_type: Some("Bearer".to_string()),
        scope: Some("offline_access".to_string()),
    };
    let request = HttpRequest {
        method: "POST".to_string(),
        url: "https://open.feishu.cn/open-apis/authen/v2/oauth/token".to_string(),
        headers: vec![],
        body: json!({
            "client_secret": CLIENT_SECRET,
            "refresh_token": REFRESH_TOKEN,
        }),
        max_response_bytes: 64,
    };
    let response = HttpResponse::new(200, success_body());
    let failure = HttpClientFailure::Transport;
    let oversized = HttpClientFailure::OversizedResponse {
        max_response_bytes: 64,
    };
    let transport_error = crate::oauth::FeishuOAuthTransportError::MaterialUnavailable;
    let transport = sample_transport(HttpResponse::new(200, success_body()));

    for rendered in [
        format!("{material:?}"),
        format!("{input:?}"),
        format!("{request:?}"),
        format!("{response:?}"),
        format!("{failure:?}"),
        failure.to_string(),
        format!("{oversized:?}"),
        oversized.to_string(),
        format!("{transport_error:?}"),
        transport_error.to_string(),
        format!("{transport:?}"),
    ] {
        assert_no_secret(&rendered);
        assert!(!rendered.contains("fp-secret"));
        assert!(!rendered.contains("access_token"));
        assert!(!rendered.contains("refresh_token"));
    }
}

#[test]
fn material_unavailable_error_debug_and_display_do_not_leak_sensitive_strings() {
    let transport_error = crate::oauth::FeishuOAuthTransportError::MaterialUnavailable;
    let rendered = [format!("{transport_error:?}"), transport_error.to_string()];
    for item in rendered {
        assert_no_secret(&item);
        assert!(!item.contains("fingerprint"));
        assert!(!item.contains("raw body"));
        assert!(!item.contains("client_secret"));
        assert!(!item.contains("refresh_token"));
        assert!(!item.contains("access_token"));
    }
}

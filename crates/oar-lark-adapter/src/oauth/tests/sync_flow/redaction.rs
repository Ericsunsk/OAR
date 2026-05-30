use serde_json::json;

use super::{
    assert_no_secret, sample_material, sample_transport, success_body, HttpClientFailure,
    HttpRequest, HttpResponse, SecretString, ACCESS_TOKEN, CLIENT_SECRET, REFRESH_TOKEN,
};

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

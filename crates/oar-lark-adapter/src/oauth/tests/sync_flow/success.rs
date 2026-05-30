use super::{
    assert_no_secret, sample_envelope, sample_request, sample_transport, success_body,
    FeishuAuthRefreshClient, FeishuAuthRefreshResponse, FeishuAuthRefreshSafeClient, HttpResponse,
    ACCESS_TOKEN, REFRESH_TOKEN,
};

#[test]
fn success_response_is_encrypted_before_core_sees_it() {
    let transport = sample_transport(HttpResponse::new(200, success_body()));
    let mut client = FeishuAuthRefreshSafeClient::new(transport);

    let response = client
        .refresh(&sample_request())
        .expect("safe envelope should parse");

    match response {
        FeishuAuthRefreshResponse::Success(success) => {
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

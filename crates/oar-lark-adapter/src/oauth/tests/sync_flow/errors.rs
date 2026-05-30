use super::{
    error_body, sample_request, sample_transport, transport_with_http_error,
    FeishuAuthRefreshClient, FeishuAuthRefreshFailure, FeishuAuthRefreshResponse,
    FeishuAuthRefreshSafeClient, HttpClientFailure, HttpResponse,
};

#[test]
fn feishu_reauth_required_codes_map_to_reauth_required() {
    for code in [20024, 20026, 20037, 20064, 20073] {
        let transport = sample_transport(HttpResponse::new(400, error_body(code)));
        let mut client = FeishuAuthRefreshSafeClient::new(transport);

        let response = client
            .refresh(&sample_request())
            .expect("safe failure envelope should parse");

        assert_eq!(
            response,
            FeishuAuthRefreshResponse::Failure(FeishuAuthRefreshFailure::ReauthRequired {
                safe_error: "invalid_grant".to_string()
            })
        );
    }
}

#[test]
fn feishu_config_codes_and_http_4xx_fallback_map_to_config_required() {
    for code in [
        20002, 20008, 20009, 20010, 20036, 20048, 20063, 20066, 20067, 20068, 20069, 20070, 20074,
    ] {
        let transport = sample_transport(HttpResponse::new(400, error_body(code)));
        let mut client = FeishuAuthRefreshSafeClient::new(transport);
        let response = client
            .refresh(&sample_request())
            .expect("safe failure envelope should parse");
        assert_eq!(
            response,
            FeishuAuthRefreshResponse::Failure(FeishuAuthRefreshFailure::ConfigRequired {
                safe_error: "refresh_config_required".to_string()
            })
        );
    }

    let unknown_4xx = [
        sample_transport(HttpResponse::new(400, error_body(29999))),
        sample_transport(HttpResponse::new(401, error_body(29999))),
        sample_transport(HttpResponse::new(403, error_body(29999))),
    ];
    for transport in unknown_4xx {
        let mut client = FeishuAuthRefreshSafeClient::new(transport);
        let response = client
            .refresh(&sample_request())
            .expect("safe failure envelope should parse");
        assert_eq!(
            response,
            FeishuAuthRefreshResponse::Failure(FeishuAuthRefreshFailure::ConfigRequired {
                safe_error: "refresh_config_required".to_string()
            })
        );
    }
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
        let mut client = FeishuAuthRefreshSafeClient::new(transport);
        let response = client
            .refresh(&sample_request())
            .expect("transient safe envelope should parse");
        assert_eq!(
            response,
            FeishuAuthRefreshResponse::Failure(FeishuAuthRefreshFailure::Transient {
                safe_error: "temporarily unavailable".to_string()
            })
        );
    }
}

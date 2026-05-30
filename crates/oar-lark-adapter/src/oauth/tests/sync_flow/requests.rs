use super::{
    assert_no_secret, sample_request, sample_transport, success_body, FeishuAuthRefreshTransport,
    FeishuOpenApiConfig, HttpResponse, CLIENT_SECRET, REFRESH_TOKEN,
};

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

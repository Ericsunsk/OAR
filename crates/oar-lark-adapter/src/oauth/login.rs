mod client;
mod config;
mod error;
mod request_builder;
mod response_parser;
mod types;

pub use client::{AsyncFeishuOAuthLogin, FeishuOAuthLoginClient};
pub use config::FeishuOAuthLoginConfig;
pub use error::{FeishuOAuthLoginConfigError, FeishuOAuthLoginError};
pub use types::{FeishuOAuthLogin, FeishuOAuthLoginToken, FeishuOAuthLoginUser};

#[cfg(test)]
mod tests {
    use crate::config::FeishuOpenApiConfig;
    use crate::oauth::http::{HttpClient, HttpClientFailure, HttpRequest, HttpResponse};

    use super::*;

    #[derive(Default)]
    struct FakeHttpClient {
        responses: Vec<Result<HttpResponse, HttpClientFailure>>,
        requests: Vec<HttpRequest>,
    }

    impl HttpClient for FakeHttpClient {
        fn post_json(&mut self, request: HttpRequest) -> Result<HttpResponse, HttpClientFailure> {
            self.send_json(request)
        }

        fn send_json(&mut self, request: HttpRequest) -> Result<HttpResponse, HttpClientFailure> {
            self.requests.push(request);
            self.responses.remove(0)
        }
    }

    fn config() -> FeishuOAuthLoginConfig {
        FeishuOAuthLoginConfig::new(
            FeishuOpenApiConfig::default(),
            "https://open.feishu.cn",
            "cli_test",
            "secret-value",
            "https://oar.example.test/auth/feishu/callback",
            Some("auth:user.id:read offline_access".to_string()),
        )
        .expect("config")
    }

    #[test]
    fn authorization_url_uses_latest_authorize_endpoint_and_encodes_query() {
        let url = config().authorization_url("state with space");

        assert_eq!(
            url,
            concat!(
                "https://open.feishu.cn/open-apis/authen/v1/authorize?",
                "client_id=cli_test",
                "&response_type=code",
                "&redirect_uri=https%3A%2F%2Foar.example.test%2Fauth%2Ffeishu%2Fcallback",
                "&state=state%20with%20space",
                "&scope=auth%3Auser.id%3Aread%20offline_access"
            )
        );
    }

    #[test]
    fn exchange_code_calls_token_then_user_info_without_debug_leaking_secrets() {
        let mut client = FeishuOAuthLoginClient::new(
            config(),
            FakeHttpClient {
                responses: vec![
                    Ok(HttpResponse::new(
                        200,
                        r#"{
                          "code": 0,
                          "access_token": "access-secret",
                          "refresh_token": "refresh-secret",
                          "expires_in": 7200,
                          "refresh_token_expires_in": 2592000,
                          "token_type": "Bearer",
                          "scope": "offline_access"
                        }"#,
                    )),
                    Ok(HttpResponse::new(
                        200,
                        r#"{
                          "code": 0,
                          "data": {
                            "name": "陈敏",
                            "open_id": "ou_123",
                            "union_id": "on_123",
                            "tenant_key": "tenant_123"
                          }
                        }"#,
                    )),
                ],
                requests: vec![],
            },
        );

        let login = client.exchange_code("code-secret").expect("login");
        assert_eq!(login.user.open_id, "ou_123");
        assert_eq!(login.user.display_name, "陈敏");
        assert_eq!(login.user.tenant_key.as_deref(), Some("tenant_123"));

        let debug = format!("{login:?}");
        assert!(!debug.contains("access-secret"));
        assert!(!debug.contains("refresh-secret"));
        assert!(!debug.contains("code-secret"));
    }
}

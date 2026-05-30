use super::support::{FakeAsyncHttpClient, ACCESS_TOKEN, CLIENT_SECRET, REFRESH_TOKEN};
use super::*;

#[test]
fn postgres_async_factory_builds_send_async_adapter_without_secret_debug() {
    fn assert_async_adapter<T: AsyncAuthRefreshAdapter + Send>(_value: &T) {}

    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("test tokio runtime should build");
    let _runtime_guard = runtime.enter();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://oar:oar@127.0.0.1:5432/oar_test")
        .expect("lazy postgres pool should not connect");
    let adapter = build_postgres_async_feishu_auth_refresh_adapter(
        pool,
        FeishuOpenApiConfig {
            base_url: "https://open.feishu.cn".to_string(),
            max_response_bytes: 1024,
            request_timeout_ms: 1_500,
            connect_timeout_ms: 500,
        },
        "cli_test",
        SecretString::new(CLIENT_SECRET),
        "key-prod-v1",
        [9; 32],
    )
    .expect("postgres async production factory should build");

    assert_async_adapter(&adapter);
    let debug = format!("{adapter:?}");
    assert!(!debug.contains(CLIENT_SECRET));
    assert!(!debug.contains("key-prod-v1"));
    assert!(!debug.contains("9, 9"));
    assert!(!debug.contains(ACCESS_TOKEN));
    assert!(!debug.contains(REFRESH_TOKEN));
}

#[test]
fn postgres_factory_with_injected_http_builds_adapter_without_network() {
    fn assert_async_adapter<T: AsyncAuthRefreshAdapter + Send>(_value: &T) {}

    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("test tokio runtime should build");
    let _runtime_guard = runtime.enter();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://oar:oar@127.0.0.1:5432/oar_test")
        .expect("lazy postgres pool should not connect");
    let adapter = build_postgres_feishu_auth_refresh_adapter_with_http(
        pool,
        FeishuOpenApiConfig {
            base_url: "https://open.feishu.cn".to_string(),
            max_response_bytes: 1024,
            request_timeout_ms: 1_500,
            connect_timeout_ms: 500,
        },
        "cli_test",
        SecretString::new(CLIENT_SECRET),
        "key-prod-v1",
        [9; 32],
        FakeAsyncHttpClient,
    )
    .expect("postgres injectable factory should build");

    assert_async_adapter(&adapter);
    let debug = format!("{adapter:?}");
    assert!(!debug.contains(CLIENT_SECRET));
    assert!(!debug.contains("key-prod-v1"));
    assert!(!debug.contains("9, 9"));
    assert!(!debug.contains(ACCESS_TOKEN));
    assert!(!debug.contains(REFRESH_TOKEN));
}

#[test]
fn postgres_async_factory_rejects_invalid_inputs_without_secret_debug() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("test tokio runtime should build");
    let _runtime_guard = runtime.enter();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://oar:oar@127.0.0.1:5432/oar_test")
        .expect("lazy postgres pool should not connect");

    let empty_client_id = build_postgres_feishu_auth_refresh_adapter_with_http(
        pool.clone(),
        FeishuOpenApiConfig::default(),
        " ",
        SecretString::new(CLIENT_SECRET),
        "key-prod-v1",
        [9; 32],
        FakeAsyncHttpClient,
    )
    .expect_err("empty client id should be rejected");
    assert_eq!(
        empty_client_id,
        FeishuAuthRefreshAdapterBuildError::EmptyClientId
    );
    assert!(!format!("{empty_client_id:?}").contains(CLIENT_SECRET));
    assert!(!empty_client_id.to_string().contains(CLIENT_SECRET));

    let empty_key_id = build_postgres_feishu_auth_refresh_adapter_with_http(
        pool,
        FeishuOpenApiConfig::default(),
        "cli_test",
        SecretString::new(CLIENT_SECRET),
        " ",
        [9; 32],
        FakeAsyncHttpClient,
    )
    .expect_err("empty grant key id should be rejected");
    assert_eq!(
        empty_key_id,
        FeishuAuthRefreshAdapterBuildError::EmptyGrantKeyId
    );
    assert!(!format!("{empty_key_id:?}").contains(CLIENT_SECRET));
    assert!(!empty_key_id.to_string().contains(CLIENT_SECRET));
}

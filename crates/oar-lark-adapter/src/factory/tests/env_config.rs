use super::*;

#[test]
fn postgres_refresh_env_config_parses_required_runtime_secrets() {
    let config = PostgresFeishuAuthRefreshEnvConfig::from_env_map(&|key| match key {
        "OAR_FEISHU_APP_ID" => Some("cli_prod".to_string()),
        "OAR_FEISHU_APP_SECRET" => Some("very-secret-value".to_string()),
        "OAR_GRANT_KEY_ID" => Some("key-prod-v1".to_string()),
        "OAR_GRANT_KEY_HEX" => Some("11".repeat(32)),
        _ => None,
    })
    .expect("env config should parse");

    assert_eq!(config.app_id, "cli_prod");
    assert_eq!(config.grant_key_id, "key-prod-v1");
    assert_eq!(config.grant_key_material, [0x11; 32]);
    assert!(!format!("{config:?}").contains("very-secret-value"));
    assert!(!format!("{config:?}").contains("key-prod-v1"));
}

#[test]
fn postgres_refresh_env_config_can_generate_dev_ephemeral_grant_key() {
    let config = PostgresFeishuAuthRefreshEnvConfig::from_env_map(&|key| match key {
        "OAR_FEISHU_APP_ID" => Some("cli_dev".to_string()),
        "OAR_FEISHU_APP_SECRET" => Some("very-secret-value".to_string()),
        "OAR_ALLOW_EPHEMERAL_GRANT_KEY" => Some("true".to_string()),
        _ => None,
    })
    .expect("dev ephemeral grant key should generate");

    assert_eq!(config.app_id, "cli_dev");
    assert!(config.grant_key_id.starts_with("dev-ephemeral-"));
    assert_ne!(config.grant_key_material, [0; 32]);
    assert!(!format!("{config:?}").contains("very-secret-value"));
    assert!(!format!("{config:?}").contains(&config.grant_key_id));
}

#[test]
fn postgres_refresh_env_config_does_not_generate_ephemeral_key_for_partial_grant_config() {
    let partial = PostgresFeishuAuthRefreshEnvConfig::from_env_map(&|key| match key {
        "OAR_FEISHU_APP_ID" => Some("cli_dev".to_string()),
        "OAR_FEISHU_APP_SECRET" => Some("very-secret-value".to_string()),
        "OAR_GRANT_KEY_ID" => Some("key-dev-v1".to_string()),
        "OAR_ALLOW_EPHEMERAL_GRANT_KEY" => Some("true".to_string()),
        _ => None,
    })
    .expect_err("partial grant config should still fail");

    assert_eq!(
        partial,
        PostgresFeishuAuthRefreshEnvConfigError::MissingGrantKeyHex
    );
}

#[test]
fn postgres_refresh_env_config_rejects_missing_or_empty_required_values() {
    let missing_secret = PostgresFeishuAuthRefreshEnvConfig::from_env_map(&|key| match key {
        "OAR_FEISHU_APP_ID" => Some("cli_prod".to_string()),
        "OAR_GRANT_KEY_ID" => Some("key-prod-v1".to_string()),
        "OAR_GRANT_KEY_HEX" => Some("11".repeat(32)),
        _ => None,
    })
    .expect_err("missing app secret should fail");
    assert_eq!(
        missing_secret,
        PostgresFeishuAuthRefreshEnvConfigError::MissingAppSecret
    );

    let empty_key_id = PostgresFeishuAuthRefreshEnvConfig::from_env_map(&|key| match key {
        "OAR_FEISHU_APP_ID" => Some("cli_prod".to_string()),
        "OAR_FEISHU_APP_SECRET" => Some("very-secret-value".to_string()),
        "OAR_GRANT_KEY_ID" => Some("   ".to_string()),
        "OAR_GRANT_KEY_HEX" => Some("11".repeat(32)),
        _ => None,
    })
    .expect_err("empty grant key id should fail");
    assert_eq!(
        empty_key_id,
        PostgresFeishuAuthRefreshEnvConfigError::MissingGrantKeyId
    );
}

#[test]
fn postgres_refresh_env_config_rejects_invalid_grant_key_hex_without_leaking_input() {
    let bad_format_value = "not-hex-sensitive-key";
    let bad_format = PostgresFeishuAuthRefreshEnvConfig::from_env_map(&|key| match key {
        "OAR_FEISHU_APP_ID" => Some("cli_prod".to_string()),
        "OAR_FEISHU_APP_SECRET" => Some("super-secret-app-secret".to_string()),
        "OAR_GRANT_KEY_ID" => Some("key-prod-v1".to_string()),
        "OAR_GRANT_KEY_HEX" => Some(bad_format_value.to_string()),
        _ => None,
    })
    .expect_err("invalid hex must fail");
    assert_eq!(
        bad_format,
        PostgresFeishuAuthRefreshEnvConfigError::InvalidGrantKeyHex
    );
    let rendered_bad_format = bad_format.to_string();
    assert!(!rendered_bad_format.contains(bad_format_value));
    assert!(!rendered_bad_format.contains("super-secret-app-secret"));

    let bad_length_value = "22".repeat(31);
    let bad_length = PostgresFeishuAuthRefreshEnvConfig::from_env_map(&|key| match key {
        "OAR_FEISHU_APP_ID" => Some("cli_prod".to_string()),
        "OAR_FEISHU_APP_SECRET" => Some("super-secret-app-secret".to_string()),
        "OAR_GRANT_KEY_ID" => Some("key-prod-v1".to_string()),
        "OAR_GRANT_KEY_HEX" => Some(bad_length_value.clone()),
        _ => None,
    })
    .expect_err("invalid key length must fail");
    assert_eq!(
        bad_length,
        PostgresFeishuAuthRefreshEnvConfigError::InvalidGrantKeyHex
    );
    let rendered_bad_length = format!("{bad_length:?} {}", bad_length);
    assert!(!rendered_bad_length.contains(&bad_length_value));
    assert!(!rendered_bad_length.contains("super-secret-app-secret"));
}

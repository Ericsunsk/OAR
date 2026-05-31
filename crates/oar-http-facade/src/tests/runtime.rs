use std::time::Duration;

use sqlx::postgres::PgPoolOptions;

use crate::persistence::FacadePersistenceRuntime;
use crate::OarHttpFacadeRuntime;

#[test]
fn runtime_disables_auth_when_env_absent_and_rejects_partial_auth_config() {
    let disabled = OarHttpFacadeRuntime::from_env_map(&|_| None).expect("disabled runtime");
    assert!(disabled.feishu_login.is_none());
    assert!(disabled.agent.is_none());
    assert!(disabled.tenant_maintenance.is_none());

    let partial = OarHttpFacadeRuntime::from_env_map(&|key| {
        (key == "OAR_FEISHU_APP_ID").then(|| "cli_test".to_string())
    })
    .expect_err("partial auth config");

    assert_eq!(
        partial.to_string(),
        "oar_feishu_auth_config_partial".to_string()
    );
    assert!(!format!("{partial:?}").contains("cli_test"));
}

#[test]
fn runtime_tenant_maintenance_is_disabled_by_default() {
    let runtime = OarHttpFacadeRuntime::from_env_map(&configured_feishu_env).expect("runtime");

    assert!(runtime.tenant_maintenance.is_none());
    assert!(!format!("{runtime:?}").contains("tenant-maintenance-test"));
}

#[test]
fn runtime_tenant_maintenance_requires_database_when_enabled() {
    let error = OarHttpFacadeRuntime::from_env_map(&configured_tenant_maintenance_env)
        .expect_err("tenant maintenance requires persistence");

    assert_eq!(
        error.to_string(),
        "oar_tenant_maintenance_database_required"
    );
    assert!(!format!("{error:?}").contains("tenant-maintenance-test"));
}

#[tokio::test]
async fn runtime_tenant_maintenance_requires_feishu_auth_when_enabled() {
    let persistence = test_persistence();
    let error = OarHttpFacadeRuntime::from_env_map_with_persistence(
        &|key| match key {
            "OAR_TENANT_MAINTENANCE_ENABLED" => Some("true".to_string()),
            "OAR_TENANT_MAINTENANCE_INSTANCE_ID" => Some("tenant-maintenance-test".to_string()),
            _ => None,
        },
        Some(persistence),
    )
    .expect_err("tenant maintenance requires Feishu auth runtime");

    assert_eq!(
        error.to_string(),
        "oar_tenant_maintenance_feishu_auth_required"
    );
}

#[tokio::test]
async fn runtime_tenant_maintenance_parses_safe_runtime_settings() {
    let persistence = test_persistence();
    let runtime = OarHttpFacadeRuntime::from_env_map_with_persistence(
        &|key| match key {
            "OAR_TENANT_MAINTENANCE_ENABLED" => Some("YES".to_string()),
            "OAR_TENANT_MAINTENANCE_INTERVAL_MS" => Some("15000".to_string()),
            "OAR_TENANT_MAINTENANCE_DUE_LOOKAHEAD_MS" => Some("90000".to_string()),
            _ => configured_tenant_maintenance_env(key),
        },
        Some(persistence),
    )
    .expect("runtime");

    let settings = runtime
        .tenant_maintenance
        .as_ref()
        .expect("tenant maintenance settings");
    assert_eq!(settings.worker.instance_id, "tenant-maintenance-test");
    assert_eq!(
        settings.runtime.tick_interval,
        Duration::from_millis(15_000)
    );
    assert_eq!(settings.worker.due_lookahead_ms, 90_000);
    assert!(!format!("{runtime:?}").contains("tenant-maintenance-test"));
}

#[tokio::test]
async fn runtime_tenant_maintenance_uses_default_schedule_settings() {
    let runtime = OarHttpFacadeRuntime::from_env_map_with_persistence(
        &configured_tenant_maintenance_env,
        Some(test_persistence()),
    )
    .expect("runtime");

    let settings = runtime
        .tenant_maintenance
        .as_ref()
        .expect("tenant maintenance settings");
    assert_eq!(settings.runtime.tick_interval, Duration::from_secs(60));
    assert_eq!(settings.worker.due_lookahead_ms, 300_000);
}

#[tokio::test]
async fn runtime_tenant_maintenance_rejects_missing_instance_or_invalid_numbers() {
    let missing_instance = OarHttpFacadeRuntime::from_env_map_with_persistence(
        &|key| match key {
            "OAR_TENANT_MAINTENANCE_ENABLED" => Some("true".to_string()),
            _ => configured_feishu_env(key),
        },
        Some(test_persistence()),
    )
    .expect_err("instance id is required");
    assert_eq!(
        missing_instance.to_string(),
        "oar_tenant_maintenance_instance_id_required"
    );

    let invalid_instance = OarHttpFacadeRuntime::from_env_map_with_persistence(
        &|key| match key {
            "OAR_TENANT_MAINTENANCE_ENABLED" => Some("true".to_string()),
            "OAR_TENANT_MAINTENANCE_INSTANCE_ID" => Some("tenant/maintenance".to_string()),
            _ => configured_tenant_maintenance_env(key),
        },
        Some(test_persistence()),
    )
    .expect_err("instance id must be a short safe id");
    assert_eq!(
        invalid_instance.to_string(),
        "oar_tenant_maintenance_config_invalid"
    );

    let invalid_interval = OarHttpFacadeRuntime::from_env_map_with_persistence(
        &|key| match key {
            "OAR_TENANT_MAINTENANCE_INTERVAL_MS" => Some("0".to_string()),
            _ => configured_tenant_maintenance_env(key),
        },
        Some(test_persistence()),
    )
    .expect_err("zero interval is invalid");
    assert_eq!(
        invalid_interval.to_string(),
        "oar_tenant_maintenance_config_invalid"
    );

    let tiny_interval = OarHttpFacadeRuntime::from_env_map_with_persistence(
        &|key| match key {
            "OAR_TENANT_MAINTENANCE_INTERVAL_MS" => Some("1".to_string()),
            _ => configured_tenant_maintenance_env(key),
        },
        Some(test_persistence()),
    )
    .expect_err("sub-second interval is invalid");
    assert_eq!(
        tiny_interval.to_string(),
        "oar_tenant_maintenance_config_invalid"
    );

    let huge_lookahead = OarHttpFacadeRuntime::from_env_map_with_persistence(
        &|key| match key {
            "OAR_TENANT_MAINTENANCE_DUE_LOOKAHEAD_MS" => Some("86400001".to_string()),
            _ => configured_tenant_maintenance_env(key),
        },
        Some(test_persistence()),
    )
    .expect_err("too-large lookahead is invalid");
    assert_eq!(
        huge_lookahead.to_string(),
        "oar_tenant_maintenance_config_invalid"
    );

    let invalid_flag = OarHttpFacadeRuntime::from_env_map_with_persistence(
        &|key| match key {
            "OAR_TENANT_MAINTENANCE_ENABLED" => Some("maybe".to_string()),
            _ => configured_feishu_env(key),
        },
        Some(test_persistence()),
    )
    .expect_err("invalid enable flag is invalid");
    assert_eq!(
        invalid_flag.to_string(),
        "oar_tenant_maintenance_config_invalid"
    );
    assert!(!format!("{invalid_flag:?}").contains("maybe"));

    let ephemeral_key = OarHttpFacadeRuntime::from_env_map_with_persistence(
        &|key| match key {
            "OAR_ALLOW_EPHEMERAL_GRANT_KEY" => Some("true".to_string()),
            _ => configured_tenant_maintenance_env(key),
        },
        Some(test_persistence()),
    )
    .expect_err("tenant maintenance requires stable grant keys");
    assert_eq!(
        ephemeral_key.to_string(),
        "oar_tenant_maintenance_config_invalid"
    );
}

#[test]
fn runtime_accepts_agent_config_without_leaking_secret() {
    let runtime = OarHttpFacadeRuntime::from_env_map(&|key| match key {
        "OAR_AGENT_OPENAI_BASE_URL" => Some("https://llm.example.test/v1".to_string()),
        "OAR_AGENT_OPENAI_API_KEY" => Some("sk-sensitive".to_string()),
        "OAR_AGENT_OPENAI_MODEL" => Some("agent-model".to_string()),
        _ => None,
    })
    .expect("runtime");

    assert!(runtime.feishu_login.is_none());
    assert!(runtime.agent.is_some());
    assert!(!format!("{runtime:?}").contains("sk-sensitive"));
}

#[test]
fn runtime_accepts_anthropic_agent_config_without_leaking_secret() {
    let runtime = OarHttpFacadeRuntime::from_env_map(&|key| match key {
        "OAR_AGENT_PROVIDER" => Some("anthropic".to_string()),
        "OAR_AGENT_ANTHROPIC_API_KEY" => Some("sk-ant-sensitive".to_string()),
        "OAR_AGENT_ANTHROPIC_MODEL" => Some("claude-sonnet-test".to_string()),
        _ => None,
    })
    .expect("runtime");

    assert!(runtime.feishu_login.is_none());
    assert!(runtime.agent.is_some());
    assert!(!format!("{runtime:?}").contains("sk-ant-sensitive"));
}

#[test]
fn runtime_rejects_partial_agent_config_without_leaking_secret() {
    let error = OarHttpFacadeRuntime::from_env_map(&|key| {
        (key == "OAR_AGENT_OPENAI_API_KEY").then(|| "sk-sensitive".to_string())
    })
    .expect_err("partial agent config");

    assert_eq!(error.to_string(), "oar_agent_config_partial");
    assert!(!format!("{error:?}").contains("sk-sensitive"));
}

#[tokio::test]
async fn async_runtime_requires_persistence_key_config_when_database_is_enabled_without_feishu_login(
) {
    let error = OarHttpFacadeRuntime::from_env_map_async(&|key| match key {
        "DATABASE_URL" => Some("postgres://oar:oar@127.0.0.1:5432/oar".to_string()),
        _ => None,
    })
    .await
    .expect_err("database-backed persistence requires grant encryption key config");

    assert_eq!(error.to_string(), "oar_persistence_config_invalid");
}

#[tokio::test]
async fn async_runtime_initializes_persistence_independently_from_feishu_login() {
    let error = OarHttpFacadeRuntime::from_env_map_async(&|key| match key {
        "DATABASE_URL" => Some("not-a-postgres-url".to_string()),
        "OAR_GRANT_KEY_ID" => Some("key-prod-v1".to_string()),
        "OAR_GRANT_KEY_HEX" => Some("11".repeat(32)),
        _ => None,
    })
    .await
    .expect_err("database-backed persistence should attempt connection without feishu login");

    assert_eq!(error.to_string(), "oar_database_connect_failed");
}

#[tokio::test]
async fn async_runtime_builds_persistence_before_tenant_maintenance_gate() {
    let error = OarHttpFacadeRuntime::from_env_map_async(&|key| match key {
        "DATABASE_URL" => Some("not-a-postgres-url".to_string()),
        "OAR_GRANT_KEY_ID" => Some("key-prod-v1".to_string()),
        "OAR_GRANT_KEY_HEX" => Some("11".repeat(32)),
        _ => configured_tenant_maintenance_env(key),
    })
    .await
    .expect_err("tenant maintenance should use database-backed async path");

    assert_eq!(error.to_string(), "oar_database_connect_failed");
}

fn configured_feishu_env(key: &str) -> Option<String> {
    match key {
        "OAR_FEISHU_APP_ID" => Some("cli_test".to_string()),
        "OAR_FEISHU_APP_SECRET" => Some("feishu-sensitive-secret".to_string()),
        "OAR_FEISHU_REDIRECT_URI" => {
            Some("https://oar.example.test/auth/feishu/callback".to_string())
        }
        _ => None,
    }
}

fn configured_tenant_maintenance_env(key: &str) -> Option<String> {
    match key {
        "OAR_TENANT_MAINTENANCE_ENABLED" => Some("true".to_string()),
        "OAR_TENANT_MAINTENANCE_INSTANCE_ID" => Some("tenant-maintenance-test".to_string()),
        _ => configured_feishu_env(key),
    }
}

fn test_persistence() -> FacadePersistenceRuntime {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://localhost/oar_unreachable")
        .expect("lazy pool");
    FacadePersistenceRuntime::new_for_test(pool, "key-test-v1".to_string(), [7; 32])
}
